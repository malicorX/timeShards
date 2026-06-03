use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, NaiveTime, Timelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use timeshards_core::permissions::{Action, Resource};
use timeshards_core::ApiError;
use uuid::Uuid;

use crate::auth::{auth_from_headers, can_manage_others, require_permission};
use crate::routes::time::{current_week_bounds, employee_for_user};
use crate::state::AppState;
use crate::validation::{ensure_no_shift_overlap, validate_interval};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/time/shift-templates",
            get(list_shift_templates).post(create_shift_template),
        )
        .route(
            "/api/v1/time/shift-templates/apply-week",
            post(apply_shift_templates_week),
        )
        .route(
            "/api/v1/time/shift-templates/{id}/deactivate",
            post(deactivate_shift_template),
        )
}

#[derive(Serialize)]
struct ShiftTemplateDto {
    id: String,
    employee_id: String,
    employee_no: String,
    employee_name: String,
    name: String,
    weekday: i32,
    starts_time: String,
    ends_time: String,
    site_id: Option<String>,
    active: bool,
}

#[derive(Deserialize)]
struct CreateShiftTemplateBody {
    employee_id: String,
    name: String,
    weekday: i32,
    starts_time: String,
    ends_time: String,
    #[serde(default)]
    site_id: Option<String>,
}

#[derive(Deserialize)]
struct TemplateListQuery {
    #[serde(default)]
    employee_id: Option<String>,
}

#[derive(Deserialize)]
struct ApplyWeekQuery {
    #[serde(default)]
    week_start: Option<String>,
    #[serde(default)]
    employee_id: Option<String>,
}

#[derive(Serialize)]
struct ApplyWeekResult {
    created: u32,
    skipped: u32,
    week_start: String,
    week_end: String,
}

async fn list_shift_templates(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<TemplateListQuery>,
) -> Result<Json<Vec<ShiftTemplateDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let filter_employee = if can_manage_others(&session) {
        q.employee_id
    } else {
        match employee_for_user(&state.db, session.user_id).await {
            Ok(eid) => Some(eid.to_string()),
            Err(_) => return Ok(Json(vec![])),
        }
    };

    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        i32,
        String,
        String,
        Option<String>,
        i64,
    )> = if let Some(eid) = filter_employee {
        sqlx::query_as(
            r#"
            SELECT t.id, t.employee_id, e.employee_no, e.display_name, t.name, t.weekday,
                   t.starts_time, t.ends_time, t.site_id, t.active
            FROM shift_templates t
            JOIN employees e ON e.id = t.employee_id
            WHERE t.employee_id = ? AND t.active = 1
            ORDER BY t.weekday, t.starts_time
            "#,
        )
        .bind(eid)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as(
            r#"
            SELECT t.id, t.employee_id, e.employee_no, e.display_name, t.name, t.weekday,
                   t.starts_time, t.ends_time, t.site_id, t.active
            FROM shift_templates t
            JOIN employees e ON e.id = t.employee_id
            WHERE t.active = 1
            ORDER BY e.employee_no, t.weekday
            "#,
        )
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, eid, eno, ename, name, wd, st, et, sid, active)| ShiftTemplateDto {
                    id,
                    employee_id: eid,
                    employee_no: eno,
                    employee_name: ename,
                    name,
                    weekday: wd,
                    starts_time: st,
                    ends_time: et,
                    site_id: sid,
                    active: active != 0,
                },
            )
            .collect(),
    ))
}

async fn create_shift_template(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateShiftTemplateBody>,
) -> Result<Json<ShiftTemplateDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Create)?;

    let mut employee_id = body.employee_id.clone();
    if !can_manage_others(&session) {
        employee_id = employee_for_user(&state.db, session.user_id)
            .await?
            .to_string();
    }

    if !(1..=7).contains(&body.weekday) {
        return Err(ApiError::bad_request("weekday: 1=Mo … 7=So"));
    }
    if parse_hm(&body.starts_time).is_none() || parse_hm(&body.ends_time).is_none() {
        return Err(ApiError::bad_request("starts_time/ends_time als HH:MM"));
    }

    let site_id = if let Some(s) = body.site_id {
        s
    } else {
        sqlx::query_scalar("SELECT id FROM sites LIMIT 1")
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .ok_or_else(|| ApiError::bad_request("Kein Standort"))?
    };

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO shift_templates (
            id, employee_id, name, weekday, starts_time, ends_time, site_id, active, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?)
        "#,
    )
    .bind(&id)
    .bind(&employee_id)
    .bind(&body.name)
    .bind(body.weekday)
    .bind(&body.starts_time)
    .bind(&body.ends_time)
    .bind(&site_id)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        i32,
        String,
        String,
        Option<String>,
        i64,
    )> = sqlx::query_as(
        r#"
        SELECT t.id, t.employee_id, e.employee_no, e.display_name, t.name, t.weekday,
               t.starts_time, t.ends_time, t.site_id, t.active
        FROM shift_templates t
        JOIN employees e ON e.id = t.employee_id
        WHERE t.id = ?
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some((id, eid, eno, ename, name, wd, st, et, sid, active)) = row else {
        return Err(ApiError::internal("Vorlage nicht geladen"));
    };
    Ok(Json(ShiftTemplateDto {
        id,
        employee_id: eid,
        employee_no: eno,
        employee_name: ename,
        name,
        weekday: wd,
        starts_time: st,
        ends_time: et,
        site_id: sid,
        active: active != 0,
    }))
}

async fn apply_shift_templates_week(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ApplyWeekQuery>,
) -> Result<Json<ApplyWeekResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Create)?;

    let filter_employee = if can_manage_others(&session) {
        q.employee_id.clone()
    } else {
        employee_for_user(&state.db, session.user_id)
            .await
            .ok()
            .map(|u| u.to_string())
    };

    let (week_start, week_end) = if let Some(ref ws) = q.week_start {
        let start = chrono::DateTime::parse_from_rfc3339(ws)
            .map_err(|_| ApiError::bad_request("week_start ungültig"))?
            .with_timezone(&Utc);
        (start, start + Duration::days(7))
    } else {
        current_week_bounds(Utc::now())
    };

    let templates: Vec<(String, String, i32, String, String, Option<String>)> =
        if let Some(ref eid) = filter_employee {
            sqlx::query_as(
                r#"
                SELECT id, employee_id, weekday, starts_time, ends_time, site_id
                FROM shift_templates WHERE active = 1 AND employee_id = ?
                "#,
            )
            .bind(eid)
            .fetch_all(&state.db)
            .await
        } else {
            sqlx::query_as(
                r#"
                SELECT id, employee_id, weekday, starts_time, ends_time, site_id
                FROM shift_templates WHERE active = 1
                "#,
            )
            .fetch_all(&state.db)
            .await
        }
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut created = 0u32;
    let mut skipped = 0u32;
    let now = Utc::now().to_rfc3339();

    for (_tid, employee_id, weekday, starts_time, ends_time, site_id) in templates {
        let Some((starts_at, ends_at)) =
            shift_bounds_for_weekday(week_start, weekday, &starts_time, &ends_time)
        else {
            skipped += 1;
            continue;
        };

        if validate_interval(&starts_at, &ends_at).is_err() {
            skipped += 1;
            continue;
        }

        if ensure_no_shift_overlap(&state.db, &employee_id, &starts_at, &ends_at, None)
            .await
            .is_err()
        {
            skipped += 1;
            continue;
        }

        let site_id = match site_id {
            Some(s) if !s.is_empty() => s,
            _ => sqlx::query_scalar("SELECT id FROM sites LIMIT 1")
                .fetch_optional(&state.db)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?
                .unwrap_or_default(),
        };
        if site_id.is_empty() {
            skipped += 1;
            continue;
        }

        let shift_id = Uuid::new_v4();
        if sqlx::query(
            r#"
            INSERT INTO shift_instances (id, employee_id, site_id, starts_at, ends_at, status, created_at)
            VALUES (?, ?, ?, ?, ?, 'planned', ?)
            "#,
        )
        .bind(shift_id.to_string())
        .bind(&employee_id)
        .bind(&site_id)
        .bind(&starts_at)
        .bind(&ends_at)
        .bind(&now)
        .execute(&state.db)
        .await
        .is_ok()
        {
            created += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(Json(ApplyWeekResult {
        created,
        skipped,
        week_start: week_start.to_rfc3339(),
        week_end: week_end.to_rfc3339(),
    }))
}

async fn deactivate_shift_template(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    let updated = sqlx::query("UPDATE shift_templates SET active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::not_found("Vorlage nicht gefunden"));
    }
    Ok(Json(serde_json::json!({ "deactivated": true })))
}

fn parse_hm(s: &str) -> Option<NaiveTime> {
    let mut p = s.split(':');
    let h: u32 = p.next()?.parse().ok()?;
    let m: u32 = p.next()?.parse().ok()?;
    NaiveTime::from_hms_opt(h, m, 0)
}

fn shift_bounds_for_weekday(
    week_start: chrono::DateTime<Utc>,
    weekday: i32,
    starts_time: &str,
    ends_time: &str,
) -> Option<(String, String)> {
    let start_hm = parse_hm(starts_time)?;
    let end_hm = parse_hm(ends_time)?;
    let monday = week_start.date_naive();
    let day = monday + Duration::days((weekday - 1) as i64);
    let tz: Tz = chrono_tz::Europe::Berlin;
    let start_naive = day.and_hms_opt(start_hm.hour(), start_hm.minute(), 0)?;
    let end_naive = day.and_hms_opt(end_hm.hour(), end_hm.minute(), 0)?;
    let start_local = start_naive.and_local_timezone(tz).single()?;
    let end_local = end_naive.and_local_timezone(tz).single()?;
    Some((start_local.to_rfc3339(), end_local.to_rfc3339()))
}
