use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post, put},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use timeshards_core::permissions::{Action, Resource};
use timeshards_core::ApiError;
use chrono::NaiveDate;
use timeshards_db::{
    copy_calendar_days, generate_work_calendar_year, parse_workday_config,
    rebuild_timesheets_for_calendar, rebuild_timesheets_for_employee_recent,
    rebuild_timesheets_for_workday_model, SettlementRuleConfig, WorkdayModelConfig,
    DEFAULT_WORK_CALENDAR_ID, REBUILD_WEEKS_CALENDAR_EDIT, REBUILD_WEEKS_COPY_OR_ASSIGN,
    REBUILD_WEEKS_DAY_OVERRIDE, REBUILD_WEEKS_WORKDAY_MODEL,
};
use uuid::Uuid;

use crate::auth::{auth_from_headers, require_permission};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/time/workday-models",
            get(list_workday_models).post(create_workday_model),
        )
        .route(
            "/api/v1/time/workday-models/{id}",
            put(update_workday_model),
        )
        .route("/api/v1/time/work-calendars", get(list_work_calendars))
        .route(
            "/api/v1/time/work-calendars/{id}/days",
            get(list_calendar_days),
        )
        .route(
            "/api/v1/time/work-calendars/{id}/days/{date}",
            put(upsert_calendar_day),
        )
        .route(
            "/api/v1/time/work-calendars/{id}/generate-year",
            post(generate_year),
        )
        .route(
            "/api/v1/time/work-calendars/{id}/copy-days",
            post(copy_days),
        )
        .route(
            "/api/v1/time/employee-work-assignments",
            get(list_assignments).post(create_assignment),
        )
        .route(
            "/api/v1/time/settlement-rules",
            get(list_settlement_rules),
        )
        .route(
            "/api/v1/time/settlement-rules/{id}",
            put(update_settlement_rule),
        )
}

#[derive(Serialize)]
struct SettlementRuleDto {
    id: String,
    name: String,
    period_kind: String,
    config: SettlementRuleConfig,
}

#[derive(Deserialize)]
struct UpdateSettlementRuleBody {
    config: SettlementRuleConfig,
}

#[derive(Serialize)]
struct WorkdayModelDto {
    id: String,
    name: String,
    description: Option<String>,
    config: WorkdayModelConfig,
}

#[derive(Deserialize)]
struct CreateWorkdayModelBody {
    name: String,
    #[serde(default)]
    description: Option<String>,
    config: WorkdayModelConfig,
}

#[derive(Deserialize)]
struct UpdateWorkdayModelBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    config: Option<WorkdayModelConfig>,
}

#[derive(Serialize)]
struct WorkCalendarDto {
    id: String,
    name: String,
    holiday_calendar_id: Option<String>,
    week_close_weekday: i32,
}

#[derive(Serialize)]
struct CalendarDayDto {
    date: String,
    workday_model_id: String,
    model_name: String,
}

#[derive(Deserialize)]
struct DaysQuery {
    from: String,
    to: String,
}

#[derive(Deserialize)]
struct GenerateYearBody {
    year: i32,
}

#[derive(Deserialize)]
struct CopyDaysBody {
    source_from: String,
    source_to: String,
    target_from: String,
}

#[derive(Serialize)]
struct CopyDaysResult {
    copied: u32,
    calendar_id: String,
}

#[derive(Deserialize)]
struct UpsertCalendarDayBody {
    workday_model_id: String,
}

#[derive(Serialize)]
struct GenerateYearResult {
    inserted: u32,
    calendar_id: String,
    year: i32,
}

#[derive(Serialize)]
struct AssignmentDto {
    id: String,
    employee_id: String,
    employee_no: String,
    employee_name: String,
    work_calendar_id: String,
    work_calendar_name: String,
    valid_from: String,
    valid_to: Option<String>,
    part_time_percent: i32,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct AssignmentListQuery {
    #[serde(default)]
    employee_id: Option<String>,
}

#[derive(Deserialize)]
struct CreateAssignmentBody {
    employee_id: String,
    work_calendar_id: String,
    valid_from: String,
    #[serde(default)]
    valid_to: Option<String>,
    #[serde(default = "default_full_time")]
    part_time_percent: i32,
    #[serde(default)]
    notes: Option<String>,
}

fn default_full_time() -> i32 {
    100
}

async fn list_settlement_rules(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<SettlementRuleDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, name, period_kind, config_json FROM settlement_rules ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, period_kind, config_json)| SettlementRuleDto {
                id,
                name,
                period_kind,
                config: serde_json::from_str(&config_json).unwrap_or_default(),
            })
            .collect(),
    ))
}

async fn update_settlement_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(rule_id): Path<String>,
    Json(body): Json<UpdateSettlementRuleBody>,
) -> Result<Json<SettlementRuleDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT id, name, period_kind FROM settlement_rules WHERE id = ?",
    )
    .bind(&rule_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    let Some((id, name, period_kind)) = row else {
        return Err(ApiError::bad_request("Abrechnungsregel nicht gefunden"));
    };

    let config_json = serde_json::to_string(&body.config)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE settlement_rules SET config_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&config_json)
    .bind(&now)
    .bind(&rule_id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(SettlementRuleDto {
        id,
        name,
        period_kind,
        config: body.config,
    }))
}

async fn list_workday_models(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkdayModelDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let rows: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, name, description, config_json FROM workday_models ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, name, description, json) in rows {
        let config = parse_workday_config(&json).map_err(|e| ApiError::internal(e.to_string()))?;
        out.push(WorkdayModelDto {
            id,
            name,
            description,
            config,
        });
    }
    Ok(Json(out))
}

async fn create_workday_model(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateWorkdayModelBody>,
) -> Result<Json<WorkdayModelDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Create)?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let json = serde_json::to_string(&body.config).map_err(|e| ApiError::internal(e.to_string()))?;
    sqlx::query(
        r#"
        INSERT INTO workday_models (id, name, description, config_json, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(&json)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(WorkdayModelDto {
        id,
        name: body.name,
        description: body.description,
        config: body.config,
    }))
}

async fn update_workday_model(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateWorkdayModelBody>,
) -> Result<Json<WorkdayModelDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    if body.name.is_none() && body.description.is_none() && body.config.is_none() {
        return Err(ApiError::bad_request("Keine Felder zum Aktualisieren"));
    }

    let (mut name, mut description, mut config_json): (String, Option<String>, String) =
        sqlx::query_as("SELECT name, description, config_json FROM workday_models WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .ok_or_else(|| ApiError::bad_request("Tagesmodell nicht gefunden"))?;
    let config_changed = body.config.is_some();
    if let Some(n) = body.name {
        name = n;
    }
    if body.description.is_some() {
        description = body.description;
    }
    if let Some(cfg) = body.config {
        config_json = serde_json::to_string(&cfg).map_err(|e| ApiError::internal(e.to_string()))?;
    }
    let config = parse_workday_config(&config_json).map_err(|e| ApiError::internal(e.to_string()))?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE workday_models SET name = ?, description = ?, config_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&name)
    .bind(&description)
    .bind(&config_json)
    .bind(&now)
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    if config_changed {
        let _ = rebuild_timesheets_for_workday_model(&state.db, &id, REBUILD_WEEKS_WORKDAY_MODEL)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    Ok(Json(WorkdayModelDto {
        id,
        name,
        description,
        config,
    }))
}

async fn list_work_calendars(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkCalendarDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let rows: Vec<(String, String, Option<String>, i32)> = sqlx::query_as(
        "SELECT id, name, holiday_calendar_id, week_close_weekday FROM work_calendars ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, holiday_calendar_id, week_close_weekday)| WorkCalendarDto {
                id,
                name,
                holiday_calendar_id,
                week_close_weekday,
            })
            .collect(),
    ))
}

async fn list_calendar_days(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(calendar_id): Path<String>,
    Query(q): Query<DaysQuery>,
) -> Result<Json<Vec<CalendarDayDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT d.date, d.workday_model_id, m.name
        FROM work_calendar_days d
        JOIN workday_models m ON m.id = d.workday_model_id
        WHERE d.calendar_id = ? AND d.date >= ? AND d.date <= ?
        ORDER BY d.date
        "#,
    )
    .bind(&calendar_id)
    .bind(&q.from)
    .bind(&q.to)
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(date, workday_model_id, model_name)| CalendarDayDto {
                date,
                workday_model_id,
                model_name,
            })
            .collect(),
    ))
}

async fn upsert_calendar_day(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((calendar_id, date)): Path<(String, String)>,
    Json(body): Json<UpsertCalendarDayBody>,
) -> Result<Json<CalendarDayDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    if date.len() != 10 || !date.as_bytes().get(4).is_some_and(|b| *b == b'-') {
        return Err(ApiError::bad_request("Datum muss YYYY-MM-DD sein"));
    }

    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_calendars WHERE id = ?")
        .bind(&calendar_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if exists == 0 {
        return Err(ApiError::bad_request("Arbeitskalender nicht gefunden"));
    }

    let model_name: String = sqlx::query_scalar("SELECT name FROM workday_models WHERE id = ?")
        .bind(&body.workday_model_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::bad_request("Tagesmodell nicht gefunden"))?;

    sqlx::query(
        r#"
        INSERT INTO work_calendar_days (calendar_id, date, workday_model_id)
        VALUES (?, ?, ?)
        ON CONFLICT(calendar_id, date) DO UPDATE SET workday_model_id = excluded.workday_model_id
        "#,
    )
    .bind(&calendar_id)
    .bind(&date)
    .bind(&body.workday_model_id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let _ = rebuild_timesheets_for_calendar(&state.db, &calendar_id, REBUILD_WEEKS_DAY_OVERRIDE)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(CalendarDayDto {
        date,
        workday_model_id: body.workday_model_id,
        model_name,
    }))
}

fn parse_ymd(s: &str) -> Result<NaiveDate, ApiError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| ApiError::bad_request("Datum muss YYYY-MM-DD sein"))
}

async fn copy_days(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(calendar_id): Path<String>,
    Json(body): Json<CopyDaysBody>,
) -> Result<Json<CopyDaysResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    let source_from = parse_ymd(&body.source_from)?;
    let source_to = parse_ymd(&body.source_to)?;
    let target_from = parse_ymd(&body.target_from)?;

    let copied = copy_calendar_days(
        &state.db,
        &calendar_id,
        source_from,
        source_to,
        target_from,
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let _ = rebuild_timesheets_for_calendar(&state.db, &calendar_id, REBUILD_WEEKS_CALENDAR_EDIT)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(CopyDaysResult {
        copied,
        calendar_id,
    }))
}

async fn generate_year(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(calendar_id): Path<String>,
    Json(body): Json<GenerateYearBody>,
) -> Result<Json<GenerateYearResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    let inserted = generate_work_calendar_year(&state.db, &calendar_id, body.year)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let _ = rebuild_timesheets_for_calendar(&state.db, &calendar_id, REBUILD_WEEKS_COPY_OR_ASSIGN)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(GenerateYearResult {
        inserted,
        calendar_id,
        year: body.year,
    }))
}

async fn list_assignments(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<AssignmentListQuery>,
) -> Result<Json<Vec<AssignmentDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        i32,
        Option<String>,
    )> = if let Some(eid) = q.employee_id {
        sqlx::query_as(
            r#"
            SELECT a.id, a.employee_id, e.employee_no, e.display_name,
                   a.work_calendar_id, c.name, a.valid_from, a.valid_to, a.part_time_percent, a.notes
            FROM employee_work_assignments a
            JOIN employees e ON e.id = a.employee_id
            JOIN work_calendars c ON c.id = a.work_calendar_id
            WHERE a.employee_id = ?
            ORDER BY a.valid_from DESC
            "#,
        )
        .bind(eid)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as(
            r#"
            SELECT a.id, a.employee_id, e.employee_no, e.display_name,
                   a.work_calendar_id, c.name, a.valid_from, a.valid_to, a.part_time_percent, a.notes
            FROM employee_work_assignments a
            JOIN employees e ON e.id = a.employee_id
            JOIN work_calendars c ON c.id = a.work_calendar_id
            ORDER BY e.employee_no, a.valid_from DESC
            "#,
        )
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    employee_id,
                    employee_no,
                    employee_name,
                    work_calendar_id,
                    work_calendar_name,
                    valid_from,
                    valid_to,
                    part_time_percent,
                    notes,
                )| AssignmentDto {
                    id,
                    employee_id,
                    employee_no,
                    employee_name,
                    work_calendar_id,
                    work_calendar_name,
                    valid_from,
                    valid_to,
                    part_time_percent,
                    notes,
                },
            )
            .collect(),
    ))
}

async fn create_assignment(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateAssignmentBody>,
) -> Result<Json<AssignmentDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Create)?;

    let cal_name: String = sqlx::query_scalar("SELECT name FROM work_calendars WHERE id = ?")
        .bind(&body.work_calendar_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::bad_request("Arbeitskalender nicht gefunden"))?;

    let (employee_no, employee_name): (String, String) = sqlx::query_as(
        "SELECT employee_no, display_name FROM employees WHERE id = ?",
    )
    .bind(&body.employee_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::bad_request("Mitarbeiter nicht gefunden"))?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO employee_work_assignments (
            id, employee_id, work_calendar_id, valid_from, valid_to, part_time_percent, notes, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&body.employee_id)
    .bind(&body.work_calendar_id)
    .bind(&body.valid_from)
    .bind(&body.valid_to)
    .bind(body.part_time_percent)
    .bind(&body.notes)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let _ = rebuild_timesheets_for_employee_recent(
        &state.db,
        &body.employee_id,
        REBUILD_WEEKS_COPY_OR_ASSIGN,
    )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(AssignmentDto {
        id,
        employee_id: body.employee_id,
        employee_no,
        employee_name,
        work_calendar_id: body.work_calendar_id,
        work_calendar_name: cal_name,
        valid_from: body.valid_from,
        valid_to: body.valid_to,
        part_time_percent: body.part_time_percent,
        notes: body.notes,
    }))
}

#[allow(dead_code)]
pub fn default_calendar_id() -> &'static str {
    DEFAULT_WORK_CALENDAR_ID
}
