use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use timeshards_core::permissions::{Action, Resource};
use timeshards_db::{
    assign_all_active_without_work_calendar, audit::write_audit, grant_default_work_calendar,
    hash_password, rebuild_timesheets_for_week, week_bounds_utc as calendar_week_bounds,
};
use timeshards_core::ApiError;
use uuid::Uuid;

use std::collections::HashSet;

use crate::access_eval::employee_inside_zone;
use crate::auth::{auth_from_headers, require_permission};
use timeshards_db::{is_block_default_passwords_enabled, is_demo_seeding_enabled};
use timeshards_hardware::hardware_adapter_active;
use crate::policy::load_active_policy;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/admin/users", get(list_users).post(create_user))
        .route(
            "/api/v1/admin/employees",
            get(list_employees).post(create_employee),
        )
        .route("/api/v1/admin/employees/{id}", patch(update_employee))
        .route(
            "/api/v1/admin/employees/{id}/deactivate",
            post(deactivate_employee),
        )
        .route("/api/v1/admin/users/{id}/disable", post(disable_user))
        .route("/api/v1/admin/users/{id}/enable", post(enable_user))
        .route(
            "/api/v1/admin/users/{id}/reset-password",
            post(reset_user_password),
        )
        .route(
            "/api/v1/admin/employees/{id}/reactivate",
            post(reactivate_employee),
        )
        .route(
            "/api/v1/admin/employees/{id}/grant-zone-access",
            post(grant_employee_zone_access),
        )
        .route(
            "/api/v1/admin/employees/{id}/grant-work-calendar",
            post(grant_employee_work_calendar),
        )
        .route("/api/v1/admin/roles", get(list_roles))
        .route("/api/v1/admin/sites", get(list_sites))
        .route("/api/v1/admin/dashboard", get(dashboard))
        .route("/api/v1/admin/foundation-fix", post(foundation_fix))
        .route("/api/v1/admin/audit", get(list_audit))
        .route("/api/v1/admin/policy", get(active_policy))
}

#[derive(Serialize)]
struct PolicyDto {
    max_daily_minutes: i64,
    max_weekly_minutes: i64,
    weekly_regular_minutes: i64,
    daily_regular_minutes: i64,
    min_break_after_minutes: i64,
    min_break_minutes: i64,
}

async fn active_policy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<PolicyDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Read)?;

    let rules = load_active_policy(&state.db).await?;
    Ok(Json(PolicyDto {
        max_daily_minutes: rules.max_daily_minutes,
        max_weekly_minutes: rules.max_weekly_minutes,
        weekly_regular_minutes: rules.weekly_regular_minutes,
        daily_regular_minutes: rules.daily_regular_minutes,
        min_break_after_minutes: rules.min_break_after_minutes,
        min_break_minutes: rules.min_break_minutes,
    }))
}

#[derive(Serialize)]
struct AuditEntryDto {
    id: String,
    actor_type: String,
    action: String,
    object_type: String,
    object_id: Option<String>,
    occurred_at: String,
    reason: Option<String>,
}

async fn list_audit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Vec<AuditEntryDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::AuditLog, Action::Read)?;

    let limit = q.limit.clamp(1, 500);
    let mut sql = String::from(
        r#"
        SELECT id, actor_type, action, object_type, object_id, occurred_at, reason
        FROM audit_log
        WHERE 1=1
        "#,
    );
    if q.object_type.is_some() {
        sql.push_str(" AND object_type = ?");
    }
    if q.action.is_some() {
        sql.push_str(" AND action = ?");
    }
    if q.actor_type.is_some() {
        sql.push_str(" AND actor_type = ?");
    }
    sql.push_str(" ORDER BY occurred_at DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, (
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        Option<String>,
    )>(&sql);
    if let Some(ot) = &q.object_type {
        query = query.bind(ot);
    }
    if let Some(act) = &q.action {
        query = query.bind(act);
    }
    if let Some(at) = &q.actor_type {
        query = query.bind(at);
    }
    query = query.bind(limit);
    let rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, actor_type, action, object_type, object_id, occurred_at, reason)| {
                    AuditEntryDto {
                        id,
                        actor_type,
                        action,
                        object_type,
                        object_id,
                        occurred_at,
                        reason,
                    }
                },
            )
            .collect(),
    ))
}

#[derive(Serialize)]
struct DoorAlertDto {
    id: String,
    name: String,
    status: String,
}

#[derive(Serialize)]
struct DashboardDto {
    pending_timesheets: i64,
    draft_timesheets: i64,
    pending_absences: i64,
    clocked_in_employees: i64,
    employees_total: i64,
    shifts_this_week: i64,
    planned_shifts_this_week: i64,
    week_start: String,
    week_end: String,
    doors_alarm: i64,
    doors_forced_open: i64,
    doors_open: i64,
    people_in_building: i64,
    door_alerts: Vec<DoorAlertDto>,
    demo_seeding_enabled: bool,
    default_password_login_blocked: bool,
    hardware_adapter: &'static str,
    /// Active employees with no work-calendar assignment valid today.
    employees_without_work_calendar: i64,
    /// Draft/rejected timesheets this week with Soll=0 despite an active calendar (rebuild recommended).
    timesheets_current_week_no_soll: i64,
}

fn dashboard_week_range() -> (String, String) {
    let (start, end) = calendar_week_bounds(Utc::now());
    (start.to_rfc3339(), end.to_rfc3339())
}

async fn dashboard(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<DashboardDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::User, Action::Read)?;

    let pending_timesheets: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM timesheets WHERE status = 'pending'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let draft_timesheets: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM timesheets WHERE status IN ('draft', 'rejected')",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let pending_absences: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM absence_requests WHERE status = 'pending'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let employees_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM employees")
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let clocked_in_employees: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM employees e
        WHERE (
            SELECT te.kind FROM time_events te
            WHERE te.employee_id = e.id
            ORDER BY te.occurred_at DESC
            LIMIT 1
        ) IN ('clock_in', 'break_start', 'break_end')
        "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let (week_start, week_end) = dashboard_week_range();
    let shifts_this_week: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM shift_instances
        WHERE status NOT IN ('cancelled')
          AND ends_at > ?
          AND starts_at < ?
        "#,
    )
    .bind(&week_start)
    .bind(&week_end)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let planned_shifts_this_week: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM shift_instances
        WHERE status = 'planned'
          AND ends_at > ?
          AND starts_at < ?
        "#,
    )
    .bind(&week_start)
    .bind(&week_end)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let doors_alarm: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM doors WHERE status = 'alarm'")
            .fetch_one(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

    let doors_forced_open: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM doors WHERE status = 'forced_open'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let doors_open: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM doors WHERE status = 'open'")
            .fetch_one(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

    let alert_rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT id, name, status FROM doors
        WHERE status IN ('alarm', 'forced_open', 'open')
        ORDER BY CASE status WHEN 'alarm' THEN 0 WHEN 'forced_open' THEN 1 ELSE 2 END, name
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let door_alerts: Vec<DoorAlertDto> = alert_rows
        .into_iter()
        .map(|(id, name, status)| DoorAlertDto { id, name, status })
        .collect();

    let zone_ids: Vec<String> = sqlx::query_scalar("SELECT id FROM zones")
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let employee_ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM employees WHERE active_to IS NULL")
            .fetch_all(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut inside_employees = HashSet::new();
    for zid in &zone_ids {
        for eid in &employee_ids {
            if employee_inside_zone(&state.db, eid, zid).await? {
                inside_employees.insert(eid.as_str());
            }
        }
    }
    let people_in_building = inside_employees.len() as i64;

    let today = Utc::now().format("%Y-%m-%d").to_string();
    let employees_without_work_calendar: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM employees e
        WHERE e.active_to IS NULL
          AND NOT EXISTS (
            SELECT 1 FROM employee_work_assignments a
            WHERE a.employee_id = e.id
              AND a.valid_from <= ?
              AND (a.valid_to IS NULL OR substr(a.valid_to, 1, 10) > ?)
          )
        "#,
    )
    .bind(&today)
    .bind(&today)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let timesheets_current_week_no_soll: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM timesheets t
        WHERE t.period_start = ?
          AND t.expected_minutes = 0
          AND t.status IN ('draft', 'rejected')
          AND EXISTS (
            SELECT 1 FROM employee_work_assignments a
            WHERE a.employee_id = t.employee_id
              AND a.valid_from <= ?
              AND (a.valid_to IS NULL OR substr(a.valid_to, 1, 10) > ?)
          )
        "#,
    )
    .bind(&week_start)
    .bind(&today)
    .bind(&today)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(DashboardDto {
        pending_timesheets,
        draft_timesheets,
        pending_absences,
        clocked_in_employees,
        employees_total,
        shifts_this_week,
        planned_shifts_this_week,
        week_start,
        week_end,
        doors_alarm,
        doors_forced_open,
        doors_open,
        people_in_building,
        door_alerts,
        demo_seeding_enabled: is_demo_seeding_enabled(),
        default_password_login_blocked: !is_demo_seeding_enabled()
            || is_block_default_passwords_enabled(),
        hardware_adapter: hardware_adapter_active(),
        employees_without_work_calendar,
        timesheets_current_week_no_soll,
    }))
}

#[derive(Serialize)]
struct UserDto {
    id: String,
    username: String,
    display_name: String,
    email: Option<String>,
    status: String,
    roles: Vec<String>,
}

#[derive(Serialize)]
struct EmployeeDto {
    id: String,
    user_id: Option<String>,
    username: Option<String>,
    employee_no: String,
    display_name: String,
    org_unit: Option<String>,
    active: bool,
    active_to: Option<String>,
    work_calendar_assigned: bool,
}

#[derive(Deserialize)]
struct UpdateEmployeeBody {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    org_unit: Option<String>,
    /// Set login user id, or empty string to unlink
    #[serde(default)]
    user_id: Option<String>,
}

#[derive(Serialize)]
struct RoleDto {
    id: String,
    name: String,
    template_key: Option<String>,
}

#[derive(Deserialize)]
struct CreateUserBody {
    username: String,
    password: String,
    display_name: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default = "default_role")]
    role_name: String,
    #[serde(default)]
    employee_no: Option<String>,
}

fn default_role() -> String {
    "employee".into()
}

#[derive(Deserialize)]
struct CreateEmployeeBody {
    display_name: String,
    #[serde(default)]
    employee_no: Option<String>,
    #[serde(default)]
    org_unit: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    /// When true, issues an active badge with credential DEMO-{employee_no}
    #[serde(default)]
    issue_badge: bool,
    /// When true, creates allow rule on Büro zone (or first zone)
    #[serde(default = "default_true")]
    grant_zone_access: bool,
    /// When true, assigns default work calendar (Sollzeit) if none exists
    #[serde(default = "default_true")]
    grant_work_calendar: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    include_inactive: bool,
}

#[derive(Deserialize)]
struct UserListQuery {
    #[serde(default)]
    include_inactive: bool,
    #[serde(default)]
    q: Option<String>,
}

#[derive(Deserialize)]
struct ResetPasswordBody {
    new_password: String,
}

#[derive(Deserialize)]
struct AuditQuery {
    #[serde(default)]
    object_type: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    actor_type: Option<String>,
    #[serde(default = "default_audit_limit")]
    limit: u32,
}

fn default_audit_limit() -> u32 {
    100
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<UserListQuery>,
) -> Result<Json<Vec<UserDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::User, Action::Read)?;

    let search = q
        .q
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"));

    let mut sql = if q.include_inactive {
        String::from("SELECT id, username, display_name, email, status FROM users")
    } else {
        String::from(
            "SELECT id, username, display_name, email, status FROM users WHERE status = 'active'",
        )
    };
    if search.is_some() {
        sql.push_str(if q.include_inactive {
            " WHERE "
        } else {
            " AND "
        });
        sql.push_str("(username LIKE ? OR display_name LIKE ?)");
    }
    sql.push_str(" ORDER BY status, username");

    let mut query = sqlx::query_as::<_, (String, String, String, Option<String>, String)>(&sql);
    if let Some(pat) = &search {
        query = query.bind(pat).bind(pat);
    }
    let rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut out = Vec::new();
    for (id, username, display_name, email, status) in rows {
        let roles: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT r.name FROM user_roles ur
            JOIN roles r ON r.id = ur.role_id
            WHERE ur.user_id = ?
            "#,
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

        out.push(UserDto {
            id,
            username,
            display_name,
            email,
            status,
            roles: roles.into_iter().map(|(n,)| n).collect(),
        });
    }
    Ok(Json(out))
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateUserBody>,
) -> Result<Json<UserDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::User, Action::Create)?;

    if body.username.len() < 2 || body.password.len() < 6 {
        return Err(ApiError::bad_request(
            "Benutzername min. 2 Zeichen, Passwort min. 6 Zeichen",
        ));
    }

    let user_id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    let hash = hash_password(&body.password).map_err(|e| ApiError::internal(e.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO users (id, username, display_name, email, password_hash, locale, status, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 'de', 'active', ?, ?)
        "#,
    )
    .bind(user_id.to_string())
    .bind(&body.username)
    .bind(&body.display_name)
    .bind(&body.email)
    .bind(&hash)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            ApiError::bad_request("Benutzername bereits vergeben")
        } else {
            ApiError::internal(e.to_string())
        }
    })?;

    let role_id: Option<String> = sqlx::query_scalar("SELECT id FROM roles WHERE name = ?")
        .bind(&body.role_name)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let role_id = role_id.ok_or_else(|| {
        ApiError::bad_request(format!("Rolle '{}' nicht gefunden", body.role_name))
    })?;

    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id.to_string())
        .bind(&role_id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let emp_no = match body.employee_no {
        Some(n) => n,
        None => format!("E{:04}", rand_emp_suffix(&state).await?),
    };

    let emp_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO employees (id, user_id, employee_no, display_name, active_from, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(emp_id.to_string())
    .bind(user_id.to_string())
    .bind(&emp_no)
    .bind(&body.display_name)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    grant_default_zone_access(&state.db, &emp_id.to_string()).await?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "create",
        "user",
        Some(user_id),
        None,
        None,
        Some(serde_json::json!({ "username": body.username })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(UserDto {
        id: user_id.to_string(),
        username: body.username,
        display_name: body.display_name,
        email: body.email,
        status: "active".into(),
        roles: vec![body.role_name],
    }))
}

async fn rand_emp_suffix(state: &AppState) -> Result<u32, ApiError> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM employees")
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((n as u32) + 1)
}

async fn list_employees(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<EmployeeDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Employee, Action::Read)?;

    let search = q
        .q
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"));

    let today = Utc::now().format("%Y-%m-%d").to_string();
    let calendar_exists = r#"
        EXISTS (
          SELECT 1 FROM employee_work_assignments a
          WHERE a.employee_id = e.id
            AND a.valid_from <= ?
            AND (a.valid_to IS NULL OR substr(a.valid_to, 1, 10) > ?)
        )
    "#;

    let mut sql = if q.include_inactive {
        format!(
            r#"
        SELECT e.id, e.user_id, u.username, e.employee_no, e.display_name, e.org_unit, e.active_to,
               CASE WHEN {calendar_exists} THEN 1 ELSE 0 END
        FROM employees e
        LEFT JOIN users u ON u.id = e.user_id
        "#
        )
    } else {
        format!(
            r#"
        SELECT e.id, e.user_id, u.username, e.employee_no, e.display_name, e.org_unit, e.active_to,
               CASE WHEN {calendar_exists} THEN 1 ELSE 0 END
        FROM employees e
        LEFT JOIN users u ON u.id = e.user_id
        WHERE e.active_to IS NULL
        "#
        )
    };
    if search.is_some() {
        sql.push_str(if q.include_inactive {
            " WHERE "
        } else {
            " AND "
        });
        sql.push_str(
            "(e.display_name LIKE ? OR e.employee_no LIKE ? OR u.username LIKE ?)",
        );
    }
    sql.push_str(" ORDER BY e.active_to IS NOT NULL, e.employee_no");

    let mut query = sqlx::query_as::<_, (
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
        i32,
    )>(&sql);
    query = query.bind(&today).bind(&today);
    if let Some(pat) = &search {
        query = query.bind(pat).bind(pat).bind(pat);
    }
    let rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    user_id,
                    username,
                    employee_no,
                    display_name,
                    org_unit,
                    active_to,
                    has_cal,
                )| {
                    EmployeeDto {
                        active: active_to.is_none(),
                        active_to,
                        id,
                        user_id,
                        username,
                        employee_no,
                        display_name,
                        org_unit,
                        work_calendar_assigned: has_cal != 0,
                    }
                },
            )
            .collect(),
    ))
}

#[derive(Serialize)]
struct FoundationFixResult {
    calendars_assigned: u32,
    timesheets_updated: u32,
    warnings: Vec<String>,
}

/// Assign missing default work calendars and rebuild current calendar week for all employees.
async fn foundation_fix(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<FoundationFixResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    let calendars_assigned = assign_all_active_without_work_calendar(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let (week_start, _) = calendar_week_bounds(Utc::now());
    let (timesheets_updated, warnings) = rebuild_timesheets_for_week(&state.db, week_start)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(FoundationFixResult {
        calendars_assigned,
        timesheets_updated,
        warnings,
    }))
}

async fn grant_default_zone_access(
    pool: &sqlx::SqlitePool,
    employee_id: &str,
) -> Result<(), ApiError> {
    let zone_id: Option<String> = match sqlx::query_scalar::<_, String>(
        "SELECT id FROM zones WHERE name = 'Büro' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    {
        Some(z) => Some(z),
        None => {
            sqlx::query_scalar("SELECT id FROM zones ORDER BY name LIMIT 1")
                .fetch_optional(pool)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?
        }
    };

    let Some(zone_id) = zone_id else {
        return Ok(());
    };

    let exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM access_rules
        WHERE principal_type = 'employee' AND principal_id = ? AND zone_id = ? AND mode = 'allow'
        "#,
    )
    .bind(employee_id)
    .bind(&zone_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    if exists > 0 {
        return Ok(());
    }

    let rule_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO access_rules (
            id, principal_type, principal_id, zone_id, door_id, schedule_json,
            valid_from, valid_to, mode, created_at
        ) VALUES (?, 'employee', ?, ?, NULL, NULL, ?, NULL, 'allow', ?)
        "#,
    )
    .bind(&rule_id)
    .bind(employee_id)
    .bind(&zone_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(())
}

async fn create_employee(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateEmployeeBody>,
) -> Result<Json<EmployeeDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Employee, Action::Create)?;

    let emp_id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    let employee_no = match body.employee_no {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => format!("E{:04}", rand_emp_suffix(&state).await?),
    };

    sqlx::query(
        r#"
        INSERT INTO employees (id, user_id, employee_no, display_name, org_unit, active_from, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(emp_id.to_string())
    .bind(&body.user_id)
    .bind(&employee_no)
    .bind(&body.display_name)
    .bind(&body.org_unit)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            ApiError::bad_request("Personalnummer bereits vergeben")
        } else {
            ApiError::internal(e.to_string())
        }
    })?;

    if body.issue_badge {
        let badge_id = Uuid::new_v4();
        let credential_uid = format!("DEMO-{employee_no}");
        sqlx::query(
            r#"
            INSERT INTO badges (id, employee_id, credential_uid, credential_type, status, issued_at)
            VALUES (?, ?, ?, 'card', 'active', ?)
            "#,
        )
        .bind(badge_id.to_string())
        .bind(emp_id.to_string())
        .bind(&credential_uid)
        .bind(&now)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    if body.grant_zone_access {
        grant_default_zone_access(&state.db, &emp_id.to_string()).await?;
    }

    if body.grant_work_calendar {
        grant_default_work_calendar(&state.db, &emp_id.to_string())
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "create",
        "employee",
        Some(emp_id),
        None,
        None,
        Some(serde_json::json!({
            "employee_no": employee_no,
            "issue_badge": body.issue_badge
        })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    fetch_employee_dto(&state.db, &emp_id.to_string()).await
}

async fn deactivate_employee(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<EmployeeDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Employee, Action::Update)?;

    let current: Option<Option<String>> =
        sqlx::query_scalar("SELECT active_to FROM employees WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some(active_to) = current else {
        return Err(ApiError::not_found("Mitarbeiter nicht gefunden"));
    };
    if active_to.is_some() {
        return Err(ApiError::bad_request("Mitarbeiter ist bereits inaktiv"));
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE employees SET active_to = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    sqlx::query(
        r#"
        UPDATE badges SET status = 'revoked', revoked_at = ?
        WHERE employee_id = ? AND status = 'active'
        "#,
    )
    .bind(&now)
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "deactivate",
        "employee",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    fetch_employee_dto(&state.db, &id).await
}

async fn disable_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<UserDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::User, Action::Update)?;

    if session.user_id.to_string() == id {
        return Err(ApiError::bad_request("Eigenes Konto kann nicht deaktiviert werden"));
    }

    let now = Utc::now().to_rfc3339();
    let updated = sqlx::query("UPDATE users SET status = 'inactive', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if updated.rows_affected() == 0 {
        return Err(ApiError::not_found("Benutzer nicht gefunden"));
    }

    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "disable",
        "user",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    fetch_user_dto(&state.db, &id).await
}

async fn enable_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<UserDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::User, Action::Update)?;

    let now = Utc::now().to_rfc3339();
    let updated = sqlx::query("UPDATE users SET status = 'active', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if updated.rows_affected() == 0 {
        return Err(ApiError::not_found("Benutzer nicht gefunden"));
    }

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "enable",
        "user",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    fetch_user_dto(&state.db, &id).await
}

async fn reset_user_password(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ResetPasswordBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::User, Action::Update)?;

    if body.new_password.len() < 6 {
        return Err(ApiError::bad_request("Passwort min. 6 Zeichen"));
    }

    let hash = hash_password(&body.new_password).map_err(|e| ApiError::internal(e.to_string()))?;
    let now = Utc::now().to_rfc3339();

    let updated = sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
        .bind(&hash)
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if updated.rows_affected() == 0 {
        return Err(ApiError::not_found("Benutzer nicht gefunden"));
    }

    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "reset_password",
        "user",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn fetch_user_dto(pool: &sqlx::SqlitePool, id: &str) -> Result<Json<UserDto>, ApiError> {
    let row: (String, String, String, Option<String>, String) = sqlx::query_as(
        "SELECT id, username, display_name, email, status FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found("Benutzer nicht gefunden"))?;

    let roles: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT r.name FROM roles r
        JOIN user_roles ur ON ur.role_id = r.id
        WHERE ur.user_id = ?
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let (id, username, display_name, email, status) = row;
    Ok(Json(UserDto {
        id,
        username,
        display_name,
        email,
        status,
        roles,
    }))
}

async fn reactivate_employee(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<EmployeeDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Employee, Action::Update)?;

    let current: Option<Option<String>> =
        sqlx::query_scalar("SELECT active_to FROM employees WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some(active_to) = current else {
        return Err(ApiError::not_found("Mitarbeiter nicht gefunden"));
    };
    if active_to.is_none() {
        return Err(ApiError::bad_request("Mitarbeiter ist bereits aktiv"));
    }

    sqlx::query("UPDATE employees SET active_to = NULL WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "reactivate",
        "employee",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    fetch_employee_dto(&state.db, &id).await
}

async fn grant_employee_zone_access(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Employee, Action::Update)?;

    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM employees WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if exists.is_none() {
        return Err(ApiError::not_found("Mitarbeiter nicht gefunden"));
    }

    grant_default_zone_access(&state.db, &id).await?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "grant_zone_access",
        "employee",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn grant_employee_work_calendar(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Employee, Action::Update)?;

    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM employees WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if exists.is_none() {
        return Err(ApiError::not_found("Mitarbeiter nicht gefunden"));
    }

    let assigned = grant_default_work_calendar(&state.db, &id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "grant_work_calendar",
        "employee",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true, "assigned": assigned })))
}

async fn fetch_employee_dto(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> Result<Json<EmployeeDto>, ApiError> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let row: (
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
        i32,
    ) = sqlx::query_as(
        r#"
        SELECT e.id, e.user_id, u.username, e.employee_no, e.display_name, e.org_unit, e.active_to,
               CASE WHEN EXISTS (
                 SELECT 1 FROM employee_work_assignments a
                 WHERE a.employee_id = e.id
                   AND a.valid_from <= ?
                   AND (a.valid_to IS NULL OR substr(a.valid_to, 1, 10) > ?)
               ) THEN 1 ELSE 0 END
        FROM employees e
        LEFT JOIN users u ON u.id = e.user_id
        WHERE e.id = ?
        "#,
    )
    .bind(&today)
    .bind(&today)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found("Mitarbeiter nicht gefunden"))?;

    let (
        id,
        user_id,
        username,
        employee_no,
        display_name,
        org_unit,
        active_to,
        has_cal,
    ) = row;
    Ok(Json(EmployeeDto {
        id,
        user_id,
        username,
        employee_no,
        display_name,
        org_unit,
        active: active_to.is_none(),
        active_to,
        work_calendar_assigned: has_cal != 0,
    }))
}

async fn update_employee(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateEmployeeBody>,
) -> Result<Json<EmployeeDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Employee, Action::Update)?;

    let exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM employees WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    if exists.is_none() {
        return Err(ApiError::not_found("Mitarbeiter nicht gefunden"));
    }

    if let Some(name) = &body.display_name {
        if name.trim().is_empty() {
            return Err(ApiError::bad_request("Anzeigename darf nicht leer sein"));
        }
        sqlx::query("UPDATE employees SET display_name = ? WHERE id = ?")
            .bind(name.trim())
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    if let Some(org) = &body.org_unit {
        let val = if org.trim().is_empty() {
            None
        } else {
            Some(org.trim().to_string())
        };
        sqlx::query("UPDATE employees SET org_unit = ? WHERE id = ?")
            .bind(val)
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    if let Some(uid) = &body.user_id {
        if uid.is_empty() {
            sqlx::query("UPDATE employees SET user_id = NULL WHERE id = ?")
                .bind(&id)
                .execute(&state.db)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        } else {
            let user_exists: Option<String> =
                sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
                    .bind(uid)
                    .fetch_optional(&state.db)
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?;
            if user_exists.is_none() {
                return Err(ApiError::bad_request("Benutzer nicht gefunden"));
            }
            let taken: Option<String> = sqlx::query_scalar(
                "SELECT id FROM employees WHERE user_id = ? AND id != ?",
            )
            .bind(uid)
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
            if taken.is_some() {
                return Err(ApiError::bad_request(
                    "Benutzer ist bereits einem anderen Mitarbeiter zugeordnet",
                ));
            }
            sqlx::query("UPDATE employees SET user_id = ? WHERE id = ?")
                .bind(uid)
                .bind(&id)
                .execute(&state.db)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        }
    }

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "update",
        "employee",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    fetch_employee_dto(&state.db, &id).await
}

async fn list_roles(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<RoleDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Role, Action::Read)?;

    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, template_key FROM roles ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, template_key)| RoleDto {
                id,
                name,
                template_key,
            })
            .collect(),
    ))
}

#[derive(Serialize)]
struct SiteDto {
    id: String,
    name: String,
    timezone: String,
}

async fn list_sites(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<SiteDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::SystemConfig, Action::Read)?;

    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id, name, timezone FROM sites ORDER BY name")
            .fetch_all(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, timezone)| SiteDto {
                id,
                name,
                timezone,
            })
            .collect(),
    ))
}
