use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use timeshards_core::{
    events::topics,
    permissions::{Action, Resource},
    ApiError, DomainEvent,
};
use uuid::Uuid;

use crate::auth::{auth_from_headers, can_manage_others, require_permission};
use crate::state::AppState;
use timeshards_db::{
    post_timesheet_approval, rebuild_timesheet_for_employee_week, rebuild_timesheets_for_week,
    week_bounds_utc,
};
use crate::validation::{ensure_no_shift_overlap, validate_interval};
use timeshards_db::audit::write_audit;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/time/clock-in", post(clock_in))
        .route("/api/v1/time/clock-out", post(clock_out))
        .route("/api/v1/time/break-start", post(break_start))
        .route("/api/v1/time/break-end", post(break_end))
        .route("/api/v1/time/events", get(list_events))
        .route("/api/v1/time/corrections", post(create_correction))
        .route("/api/v1/time/status", get(status))
        .route("/api/v1/time/calendar-week", get(calendar_week))
        .route("/api/v1/time/clocked-in", get(list_clocked_in))
        .route("/api/v1/time/shifts", get(list_shifts).post(create_shift))
        .route("/api/v1/time/shifts/{id}/publish", post(publish_shift))
        .route("/api/v1/time/shifts/publish-planned", post(publish_planned_shifts))
        .route("/api/v1/time/shifts/{id}/cancel", post(cancel_shift))
        .route("/api/v1/time/timesheets", get(list_timesheets))
        .route("/api/v1/time/timesheets/rebuild", post(rebuild_timesheets))
        .route("/api/v1/time/timesheets/{id}/submit", post(submit_timesheet))
        .route("/api/v1/time/timesheets/{id}/approve", post(approve_timesheet))
        .route(
            "/api/v1/time/timesheets/submit-drafts",
            post(submit_draft_timesheets),
        )
        .route(
            "/api/v1/time/timesheets/approve-pending",
            post(approve_pending_timesheets),
        )
        .route("/api/v1/time/timesheets/{id}/reject", post(reject_timesheet))
}

#[derive(Serialize)]
struct ClockResponse {
    event_id: Uuid,
    kind: String,
    occurred_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    advisory: Option<String>,
}

#[derive(Serialize)]
struct TimeStatus {
    employee_id: Option<Uuid>,
    last_kind: Option<String>,
    is_clocked_in: bool,
    is_on_break: bool,
}

fn can_correct_time(session: &timeshards_core::AuthSession) -> bool {
    let ps = &session.permissions;
    ps.allows(Resource::TimeEvent, Action::Override)
        || ps.allows(Resource::TimeEvent, Action::Approve)
}

async fn last_event_kind(
    pool: &sqlx::SqlitePool,
    employee_id: &str,
) -> Result<Option<String>, ApiError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT kind FROM time_events WHERE employee_id = ? ORDER BY occurred_at DESC LIMIT 1",
    )
    .bind(employee_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(row.map(|(k,)| k))
}

fn validate_time_transition(last: Option<&str>, next: &str) -> Result<(), ApiError> {
    let ok = match (last, next) {
        (None, "clock_in") => true,
        (Some("clock_out") | Some("break_end"), "clock_in") => true,
        (Some("clock_in") | Some("break_end"), "break_start") => true,
        (Some("break_start"), "break_end") => true,
        (Some("clock_in") | Some("break_end"), "clock_out") => true,
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "Ungültige Reihenfolge: {:?} → {}",
            last, next
        )))
    }
}

pub(crate) async fn employee_for_user(
    pool: &sqlx::SqlitePool,
    user_id: Uuid,
) -> Result<Uuid, ApiError> {
    let id: Option<String> = sqlx::query_scalar("SELECT id FROM employees WHERE user_id = ?")
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    id.map(|s| Uuid::parse_str(&s).map_err(|e| ApiError::internal(e.to_string())))
        .transpose()?
        .ok_or_else(|| ApiError::bad_request("Kein Mitarbeiterprofil für diesen Benutzer"))
}

async fn clock_in(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ClockResponse>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::TimeEvent, Action::Create)?;
    let employee_id = employee_for_user(&state.db, session.user_id).await?;
    record_time_event(
        &state,
        employee_id,
        "clock_in",
        topics::TIME_CLOCK_IN,
        "client",
        None,
        None,
    )
    .await
}

async fn clock_out(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ClockResponse>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::TimeEvent, Action::Create)?;
    let employee_id = employee_for_user(&state.db, session.user_id).await?;
    record_time_event(
        &state,
        employee_id,
        "clock_out",
        topics::TIME_CLOCK_OUT,
        "client",
        None,
        None,
    )
    .await
}

async fn break_start(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ClockResponse>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::TimeEvent, Action::Create)?;
    let employee_id = employee_for_user(&state.db, session.user_id).await?;
    record_time_event(
        &state,
        employee_id,
        "break_start",
        topics::TIME_BREAK_START,
        "client",
        None,
        None,
    )
    .await
}

async fn break_end(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ClockResponse>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::TimeEvent, Action::Create)?;
    let employee_id = employee_for_user(&state.db, session.user_id).await?;
    record_time_event(
        &state,
        employee_id,
        "break_end",
        topics::TIME_BREAK_END,
        "client",
        None,
        None,
    )
    .await
}

fn topic_for_kind(kind: &str) -> &'static str {
    match kind {
        "clock_out" => topics::TIME_CLOCK_OUT,
        "break_start" => topics::TIME_BREAK_START,
        "break_end" => topics::TIME_BREAK_END,
        _ => topics::TIME_CLOCK_IN,
    }
}

#[derive(Deserialize)]
struct CorrectionBody {
    employee_id: String,
    kind: String,
    occurred_at: String,
    reason: String,
}

async fn create_correction(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CorrectionBody>,
) -> Result<Json<ClockResponse>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    if !can_correct_time(&session) {
        return Err(ApiError::forbidden(
            "Keine Berechtigung für Zeitkorrekturen",
        ));
    }
    if !matches!(
        body.kind.as_str(),
        "clock_in" | "clock_out" | "break_start" | "break_end"
    ) {
        return Err(ApiError::bad_request("Ungültiger Ereignistyp"));
    }
    if body.reason.trim().len() < 3 {
        return Err(ApiError::bad_request(
            "Begründung mindestens 3 Zeichen",
        ));
    }
    let emp = Uuid::parse_str(&body.employee_id)
        .map_err(|_| ApiError::bad_request("Ungültige employee_id"))?;
    let notes = Some(body.reason.as_str());
    let topic = topic_for_kind(&body.kind);
    let resp = record_time_event(
        &state,
        emp,
        &body.kind,
        topic,
        "correction",
        notes,
        Some(&body.occurred_at),
    )
    .await?;
    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "correct",
        "time_event",
        Some(resp.event_id),
        Some(&body.reason),
        None,
        Some(serde_json::json!({
            "employee_id": body.employee_id,
            "kind": body.kind,
            "occurred_at": body.occurred_at
        })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(resp)
}

#[derive(Deserialize)]
struct EventListQuery {
    #[serde(default)]
    employee_id: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

async fn record_time_event(
    state: &AppState,
    employee_id: Uuid,
    kind: &str,
    topic: &str,
    source: &str,
    notes: Option<&str>,
    occurred_at_override: Option<&str>,
) -> Result<Json<ClockResponse>, ApiError> {
    let eid = employee_id.to_string();
    let last = last_event_kind(&state.db, &eid).await?;
    if source != "correction" {
        validate_time_transition(last.as_deref(), kind)?;
    }

    let event_id = Uuid::new_v4();
    let occurred_at = if let Some(raw) = occurred_at_override {
        DateTime::parse_from_rfc3339(raw)
            .map_err(|_| ApiError::bad_request("Ungültiges occurred_at (RFC3339)"))?
            .with_timezone(&Utc)
    } else {
        Utc::now()
    };
    let now = occurred_at.to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO time_events (id, employee_id, kind, occurred_at, source, notes, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(event_id.to_string())
    .bind(&eid)
    .bind(kind)
    .bind(&now)
    .bind(source)
    .bind(notes)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let domain = DomainEvent {
        id: Uuid::new_v4(),
        topic: topic.to_string(),
        schema_version: 1,
        occurred_at,
        producer: "shard.time".into(),
        correlation_id: Some(event_id),
        actor: None,
        payload: serde_json::json!({
            "employee_id": employee_id,
            "time_event_id": event_id,
            "kind": kind
        }),
    };

    sqlx::query(
        r#"
        INSERT INTO domain_events (id, topic, schema_version, occurred_at, producer, correlation_id, payload_json)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(domain.id.to_string())
    .bind(&domain.topic)
    .bind(1i64)
    .bind(domain.occurred_at.to_rfc3339())
    .bind(&domain.producer)
    .bind(domain.correlation_id.map(|u| u.to_string()))
    .bind(domain.payload.to_string())
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    if matches!(kind, "clock_out" | "break_end") {
        let (week_start, _) = timeshards_db::week_bounds_utc(occurred_at);
        let _ = timeshards_db::rebuild_timesheet_for_employee_week(&state.db, &eid, week_start).await;
    }

    let advisory = if source != "correction" && (kind == "clock_in" || kind == "clock_out") {
        match timeshards_db::punch_flex_check(&state.db, &eid, kind, occurred_at).await {
            Ok(check) => {
                if check.enforce {
                    if let Some(msg) = check.advisory {
                        return Err(ApiError::bad_request(msg));
                    }
                }
                check.advisory
            }
            Err(_) => None,
        }
    } else {
        None
    };

    Ok(Json(ClockResponse {
        event_id,
        kind: kind.to_string(),
        occurred_at: now,
        advisory,
    }))
}

async fn list_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<EventListQuery>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::TimeEvent, Action::Read)?;

    let limit = q.limit.unwrap_or(50).min(200);

    if q.employee_id.is_none() && can_manage_others(&session) {
        let rows: Vec<(
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            String,
        )> = sqlx::query_as(
            r#"
            SELECT te.id, te.kind, te.occurred_at, te.notes, te.source,
                   e.employee_no, e.display_name
            FROM time_events te
            JOIN employees e ON e.id = te.employee_id
            ORDER BY te.occurred_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

        let items = rows
            .into_iter()
            .map(
                |(id, kind, occurred_at, notes, source, employee_no, employee_name)| {
                    serde_json::json!({
                        "id": id,
                        "kind": kind,
                        "occurred_at": occurred_at,
                        "notes": notes,
                        "source": source,
                        "employee_no": employee_no,
                        "employee_name": employee_name
                    })
                },
            )
            .collect();
        return Ok(Json(items));
    }

    let employee_id = if let Some(eid) = q.employee_id {
        if !can_manage_others(&session) {
            let own = employee_for_user(&state.db, session.user_id).await?;
            if own.to_string() != eid {
                return Err(ApiError::forbidden(
                    "Nur eigene Stempelungen einsehbar",
                ));
            }
        }
        eid
    } else {
        employee_for_user(&state.db, session.user_id)
            .await?
            .to_string()
    };

    let rows: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT id, kind, occurred_at, notes, source
        FROM time_events WHERE employee_id = ?
        ORDER BY occurred_at DESC LIMIT ?
        "#,
    )
    .bind(&employee_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let items = rows
        .into_iter()
        .map(|(id, kind, occurred_at, notes, source)| {
            serde_json::json!({
                "id": id,
                "kind": kind,
                "occurred_at": occurred_at,
                "notes": notes,
                "source": source
            })
        })
        .collect();

    Ok(Json(items))
}

#[derive(Deserialize)]
struct CalendarWeekQuery {
    /// RFC3339 instant; week containing this timestamp (default: now).
    at: Option<String>,
}

#[derive(Serialize)]
struct CalendarWeekDto {
    period_start: String,
    period_end: String,
}

/// Berlin calendar week `[period_start, period_end)` — same bounds as timesheets and dashboard.
async fn calendar_week(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<CalendarWeekQuery>,
) -> Result<Json<CalendarWeekDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::TimeEvent, Action::Read)?;

    let at = if let Some(ref raw) = q.at {
        DateTime::parse_from_rfc3339(raw)
            .map_err(|_| ApiError::bad_request("at ungültig (RFC3339 erwartet)"))?
            .with_timezone(&Utc)
    } else {
        Utc::now()
    };
    let (start, end) = week_bounds_utc(at);
    Ok(Json(CalendarWeekDto {
        period_start: start.to_rfc3339(),
        period_end: end.to_rfc3339(),
    }))
}

async fn status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<TimeStatus>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    let employee_id = employee_for_user(&state.db, session.user_id).await.ok();

    let last: Option<(String,)> = if let Some(eid) = employee_id {
        sqlx::query_as(
            "SELECT kind FROM time_events WHERE employee_id = ? ORDER BY occurred_at DESC LIMIT 1",
        )
        .bind(eid.to_string())
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    } else {
        None
    };

    let last_kind = last.map(|(k,)| k);
    let is_on_break = last_kind.as_deref() == Some("break_start");
    let is_clocked_in = matches!(
        last_kind.as_deref(),
        Some("clock_in") | Some("break_start") | Some("break_end")
    );

    Ok(Json(TimeStatus {
        employee_id,
        last_kind,
        is_clocked_in,
        is_on_break,
    }))
}

#[derive(Serialize)]
struct ClockedInDto {
    employee_id: String,
    employee_no: String,
    display_name: String,
    last_kind: String,
    last_at: String,
    is_on_break: bool,
}

async fn list_clocked_in(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<ClockedInDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::TimeEvent, Action::Read)?;

    let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT e.id, e.employee_no, e.display_name,
               (SELECT te.kind FROM time_events te
                WHERE te.employee_id = e.id
                ORDER BY te.occurred_at DESC LIMIT 1) AS kind,
               (SELECT te.occurred_at FROM time_events te
                WHERE te.employee_id = e.id
                ORDER BY te.occurred_at DESC LIMIT 1) AS occurred_at
        FROM employees e
        WHERE e.active_to IS NULL
          AND (
            SELECT te.kind FROM time_events te
            WHERE te.employee_id = e.id
            ORDER BY te.occurred_at DESC LIMIT 1
          ) IN ('clock_in', 'break_start', 'break_end')
        ORDER BY e.employee_no
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(employee_id, employee_no, display_name, last_kind, last_at)| ClockedInDto {
                employee_id,
                employee_no,
                display_name,
                is_on_break: last_kind == "break_start",
                last_kind,
                last_at,
            })
            .collect(),
    ))
}

#[derive(Serialize)]
struct ShiftDto {
    id: String,
    employee_id: String,
    employee_no: String,
    employee_name: String,
    site_id: String,
    starts_at: String,
    ends_at: String,
    status: String,
}

#[derive(Deserialize)]
struct CreateShiftBody {
    employee_id: String,
    starts_at: String,
    ends_at: String,
    #[serde(default)]
    site_id: Option<String>,
}

#[derive(Deserialize)]
struct ShiftListQuery {
    #[serde(default)]
    employee_id: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

const SHIFT_SELECT: &str = r#"
    SELECT s.id, s.employee_id, e.employee_no, e.display_name, s.site_id, s.starts_at, s.ends_at, s.status
    FROM shift_instances s
    JOIN employees e ON e.id = s.employee_id
"#;

async fn list_shifts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ShiftListQuery>,
) -> Result<Json<Vec<ShiftDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let show_all = can_manage_others(&session);
    let employee_filter = if let Some(eid) = q.employee_id {
        Some(eid)
    } else if !show_all {
        Some(employee_for_user(&state.db, session.user_id).await?.to_string())
    } else {
        None
    };

    let mut sql = SHIFT_SELECT.to_string();
    let mut conditions = Vec::new();
    if employee_filter.is_some() {
        conditions.push("s.employee_id = ?");
    }
    if q.from.is_some() {
        conditions.push("s.ends_at > ?");
    }
    if q.to.is_some() {
        conditions.push("s.starts_at < ?");
    }
    if q.status.is_some() {
        conditions.push("s.status = ?");
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY s.starts_at ASC LIMIT 200");

    let mut query = sqlx::query_as::<_, (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    )>(&sql);
    if let Some(eid) = employee_filter {
        query = query.bind(eid);
    }
    if let Some(from) = q.from {
        query = query.bind(from);
    }
    if let Some(to) = q.to {
        query = query.bind(to);
    }
    if let Some(st) = q.status {
        query = query.bind(st);
    }
    let rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, employee_id, employee_no, employee_name, site_id, starts_at, ends_at, status)| {
                    ShiftDto {
                        id,
                        employee_id,
                        employee_no,
                        employee_name,
                        site_id,
                        starts_at,
                        ends_at,
                        status,
                    }
                },
            )
            .collect(),
    ))
}

async fn create_shift(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateShiftBody>,
) -> Result<Json<ShiftDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Create)?;

    validate_interval(&body.starts_at, &body.ends_at)?;
    ensure_no_shift_overlap(
        &state.db,
        &body.employee_id,
        &body.starts_at,
        &body.ends_at,
        None,
    )
    .await?;

    let site_id = if let Some(s) = body.site_id {
        s
    } else {
        sqlx::query_scalar("SELECT id FROM sites LIMIT 1")
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .ok_or_else(|| ApiError::bad_request("Kein Standort konfiguriert"))?
    };

    let id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO shift_instances (id, employee_id, site_id, starts_at, ends_at, status, created_at)
        VALUES (?, ?, ?, ?, ?, 'planned', ?)
        "#,
    )
    .bind(id.to_string())
    .bind(&body.employee_id)
    .bind(&site_id)
    .bind(&body.starts_at)
    .bind(&body.ends_at)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let (employee_no, employee_name): (String, String) = sqlx::query_as(
        "SELECT employee_no, display_name FROM employees WHERE id = ?",
    )
    .bind(&body.employee_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(ShiftDto {
        id: id.to_string(),
        employee_id: body.employee_id,
        employee_no,
        employee_name,
        site_id,
        starts_at: body.starts_at,
        ends_at: body.ends_at,
        status: "planned".into(),
    }))
}

async fn fetch_shift(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> Result<ShiftDto, ApiError> {
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    )> = sqlx::query_as(&format!("{SHIFT_SELECT} WHERE s.id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let Some((
        id,
        employee_id,
        employee_no,
        employee_name,
        site_id,
        starts_at,
        ends_at,
        status,
    )) = row
    else {
        return Err(ApiError::not_found("Schicht nicht gefunden"));
    };
    Ok(ShiftDto {
        id,
        employee_id,
        employee_no,
        employee_name,
        site_id,
        starts_at,
        ends_at,
        status,
    })
}

async fn publish_shift(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ShiftDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;
    let status: String = sqlx::query_scalar("SELECT status FROM shift_instances WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Schicht nicht gefunden"))?;
    if status != "planned" {
        return Err(ApiError::bad_request(
            "Nur geplante Schichten können veröffentlicht werden",
        ));
    }
    sqlx::query("UPDATE shift_instances SET status = 'published' WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "publish",
        "shift_instance",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    fetch_shift(&state.db, &id).await.map(Json)
}

#[derive(Deserialize)]
struct PublishPlannedQuery {
    #[serde(default)]
    week_start: Option<String>,
}

#[derive(Serialize)]
struct PublishPlannedResult {
    published: u64,
    week_start: String,
    week_end: String,
}

async fn publish_planned_shifts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<PublishPlannedQuery>,
) -> Result<Json<PublishPlannedResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    let (week_start, week_end) = if let Some(ref ws) = q.week_start {
        let start = chrono::DateTime::parse_from_rfc3339(ws)
            .map_err(|_| ApiError::bad_request("week_start ungültig"))?
            .with_timezone(&Utc);
        let end = start + Duration::days(7);
        (start, end)
    } else {
        current_week_bounds(Utc::now())
    };

    let result = sqlx::query(
        r#"
        UPDATE shift_instances
        SET status = 'published'
        WHERE status = 'planned'
          AND starts_at >= ?
          AND starts_at < ?
        "#,
    )
    .bind(week_start.to_rfc3339())
    .bind(week_end.to_rfc3339())
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "publish_planned",
        "shift_instance",
        None,
        None,
        None,
        Some(serde_json::json!({ "published": result.rows_affected() })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(PublishPlannedResult {
        published: result.rows_affected(),
        week_start: week_start.to_rfc3339(),
        week_end: week_end.to_rfc3339(),
    }))
}

async fn cancel_shift(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ShiftDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;
    let status: String = sqlx::query_scalar("SELECT status FROM shift_instances WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Schicht nicht gefunden"))?;
    if status == "cancelled" {
        return Err(ApiError::bad_request("Schicht ist bereits storniert"));
    }
    sqlx::query("UPDATE shift_instances SET status = 'cancelled' WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "cancel",
        "shift_instance",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    fetch_shift(&state.db, &id).await.map(Json)
}

#[derive(Serialize)]
struct TimesheetDto {
    id: String,
    employee_id: String,
    employee_no: String,
    employee_name: String,
    period_start: String,
    period_end: String,
    worked_minutes: i64,
    expected_minutes: i64,
    balance_minutes: i64,
    overtime_minutes: i64,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rejection_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evaluation: Option<timeshards_db::WeekEvaluationMeta>,
}

#[derive(Deserialize)]
struct RejectTimesheetBody {
    #[serde(default)]
    reason: Option<String>,
}

struct TimesheetRow {
    id: String,
    employee_id: String,
    employee_no: String,
    employee_name: String,
    period_start: String,
    period_end: String,
    worked_minutes: i64,
    expected_minutes: i64,
    balance_minutes: i64,
    overtime_minutes: i64,
    status: String,
    rejection_reason: Option<String>,
    evaluation_json: Option<String>,
}

impl From<TimesheetRow> for TimesheetDto {
    fn from(r: TimesheetRow) -> Self {
        let evaluation = r
            .evaluation_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok());
        Self {
            id: r.id,
            employee_id: r.employee_id,
            employee_no: r.employee_no,
            employee_name: r.employee_name,
            period_start: r.period_start,
            period_end: r.period_end,
            worked_minutes: r.worked_minutes,
            expected_minutes: r.expected_minutes,
            balance_minutes: r.balance_minutes,
            overtime_minutes: r.overtime_minutes,
            status: r.status,
            rejection_reason: r.rejection_reason,
            evaluation,
        }
    }
}

const TS_SELECT: &str = r#"
    SELECT t.id, t.employee_id, e.employee_no, e.display_name, t.period_start, t.period_end,
           t.worked_minutes, t.expected_minutes, t.balance_minutes, t.overtime_minutes, t.status, t.rejection_reason,
           t.evaluation_json
    FROM timesheets t
    JOIN employees e ON e.id = t.employee_id
"#;

#[derive(Deserialize)]
struct TimesheetQuery {
    #[serde(default)]
    employee_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    period_start: Option<String>,
}

async fn list_timesheets(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<TimesheetQuery>,
) -> Result<Json<Vec<TimesheetDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Read)?;

    let show_all = can_manage_others(&session);

    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        String,
        Option<String>,
        Option<String>,
    )> = {
        let mut sql = TS_SELECT.to_string();
        let mut conditions = Vec::new();
        let employee_filter = if let Some(eid) = q.employee_id {
            Some(eid)
        } else if !show_all {
            Some(employee_for_user(&state.db, session.user_id).await?.to_string())
        } else {
            None
        };
        if employee_filter.is_some() {
            conditions.push("t.employee_id = ?");
        }
        if q.status.is_some() {
            conditions.push("t.status = ?");
        }
        if q.period_start.is_some() {
            conditions.push("t.period_start = ?");
        }
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY t.period_start DESC LIMIT 50");
        let mut query = sqlx::query_as(&sql);
        if let Some(eid) = employee_filter {
            query = query.bind(eid);
        }
        if let Some(st) = q.status {
            query = query.bind(st);
        }
        if let Some(ref ps) = q.period_start {
            query = query.bind(ps);
        }
        query.fetch_all(&state.db).await
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
                    period_start,
                    period_end,
                    worked_minutes,
                    expected_minutes,
                    balance_minutes,
                    overtime_minutes,
                    status,
                    rejection_reason,
                    evaluation_json,
                )| {
                    TimesheetDto::from(TimesheetRow {
                        id,
                        employee_id,
                        employee_no,
                        employee_name,
                        period_start,
                        period_end,
                        worked_minutes,
                        expected_minutes,
                        balance_minutes,
                        overtime_minutes,
                        status,
                        rejection_reason,
                        evaluation_json,
                    })
                },
            )
            .collect(),
    ))
}

async fn load_timesheet(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> Result<(String, String), ApiError> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT employee_id, status FROM timesheets WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    row.ok_or_else(|| ApiError::not_found("Stundenzettel nicht gefunden"))
}

async fn settle_approved_timesheet(pool: &sqlx::SqlitePool, id: &str) {
    let Ok(row) = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT employee_id, period_start, balance_minutes, overtime_minutes FROM timesheets WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    else {
        return;
    };
    let Some((employee_id, period_start, balance_minutes, overtime_minutes)) = row else {
        return;
    };
    let _ = post_timesheet_approval(
        pool,
        id,
        &employee_id,
        &period_start,
        balance_minutes,
        overtime_minutes,
    )
    .await;
}

async fn submit_timesheet(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<TimesheetDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    let (employee_id, status) = load_timesheet(&state.db, &id).await?;

    let own = employee_for_user(&state.db, session.user_id).await.ok();
    let is_own = own.map(|e| e.to_string()) == Some(employee_id.clone());
    if is_own {
        require_permission(&session, Resource::Timesheet, Action::Update)?;
    } else {
        require_permission(&session, Resource::Timesheet, Action::Approve)?;
    }

    if !matches!(status.as_str(), "draft" | "rejected") {
        return Err(ApiError::bad_request(
            "Nur Entwürfe oder abgelehnte Stundenzettel können eingereicht werden",
        ));
    }

    sqlx::query(
        "UPDATE timesheets SET status = 'pending', rejection_reason = NULL, approved_by = NULL, approved_at = NULL WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    fetch_timesheet_dto(&state.db, &id).await
}

#[derive(Serialize)]
struct BulkApproveResult {
    approved: u64,
}

#[derive(Serialize)]
struct BulkSubmitResult {
    submitted: u64,
}

#[derive(Deserialize)]
struct SubmitDraftsQuery {
    #[serde(default)]
    period_start: Option<String>,
}

async fn submit_draft_timesheets(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<SubmitDraftsQuery>,
) -> Result<Json<BulkSubmitResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;

    let show_all = can_manage_others(&session);
    if show_all {
        require_permission(&session, Resource::Timesheet, Action::Approve)?;
    } else {
        require_permission(&session, Resource::Timesheet, Action::Update)?;
    }

    let mut sql =
        String::from("SELECT id FROM timesheets WHERE status IN ('draft', 'rejected')");
    if q.period_start.is_some() {
        sql.push_str(" AND period_start = ?");
    }
    if !show_all {
        sql.push_str(" AND employee_id = ?");
    }

    let mut query = sqlx::query_scalar::<_, String>(&sql);
    if let Some(ref ps) = q.period_start {
        query = query.bind(ps);
    }
    if !show_all {
        let eid = employee_for_user(&state.db, session.user_id).await?.to_string();
        query = query.bind(eid);
    }

    let ids = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut submitted = 0u64;
    for id in ids {
        if let Ok(r) = sqlx::query(
            "UPDATE timesheets SET status = 'pending', rejection_reason = NULL, approved_by = NULL, approved_at = NULL WHERE id = ?",
        )
        .bind(&id)
        .execute(&state.db)
        .await
        {
            if r.rows_affected() > 0 {
                submitted += 1;
            }
        }
    }

    Ok(Json(BulkSubmitResult { submitted }))
}

async fn approve_pending_timesheets(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<BulkApproveResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Approve)?;

    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM timesheets WHERE status = 'pending'")
            .fetch_all(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

    let now = Utc::now().to_rfc3339();
    let mut approved = 0u64;
    for id in ids {
        if set_timesheet_status(
            &state.db,
            &id,
            "approved",
            Some(session.user_id.to_string()),
            Some(now.clone()),
        )
        .await
        .is_ok()
        {
            approved += 1;
            settle_approved_timesheet(&state.db, &id).await;
            if let Ok(ts_uuid) = Uuid::parse_str(&id) {
                let _ = write_audit(
                    &state.db,
                    "user",
                    Some(session.user_id),
                    "approve",
                    "timesheet",
                    Some(ts_uuid),
                    None,
                    None,
                    None,
                )
                .await;
            }
        }
    }

    Ok(Json(BulkApproveResult { approved }))
}

async fn approve_timesheet(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<TimesheetDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Approve)?;
    let (_, status) = load_timesheet(&state.db, &id).await?;
    if status != "pending" {
        return Err(ApiError::bad_request("Nur eingereichte Stundenzettel können freigegeben werden"));
    }
    let now = Utc::now().to_rfc3339();
    set_timesheet_status(
        &state.db,
        &id,
        "approved",
        Some(session.user_id.to_string()),
        Some(now),
    )
    .await?;
    settle_approved_timesheet(&state.db, &id).await;
    let ts_uuid = Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4());
    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "approve",
        "timesheet",
        Some(ts_uuid),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    fetch_timesheet_dto(&state.db, &id).await
}

async fn reject_timesheet(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RejectTimesheetBody>,
) -> Result<Json<TimesheetDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Approve)?;
    let (_, status) = load_timesheet(&state.db, &id).await?;
    if status != "pending" {
        return Err(ApiError::bad_request("Nur eingereichte Stundenzettel können abgelehnt werden"));
    }
    sqlx::query(
        r#"
        UPDATE timesheets SET status = 'rejected', rejection_reason = ?, approved_by = NULL, approved_at = NULL
        WHERE id = ?
        "#,
    )
    .bind(&body.reason)
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    fetch_timesheet_dto(&state.db, &id).await
}

async fn set_timesheet_status(
    pool: &sqlx::SqlitePool,
    id: &str,
    status: &str,
    approved_by: Option<String>,
    approved_at: Option<String>,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        UPDATE timesheets SET status = ?, approved_by = ?, approved_at = ?,
        rejection_reason = CASE WHEN ? = 'rejected' THEN rejection_reason ELSE NULL END
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(approved_by)
    .bind(approved_at)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}

async fn fetch_timesheet_dto(pool: &sqlx::SqlitePool, id: &str) -> Result<Json<TimesheetDto>, ApiError> {
    let sql = format!("{TS_SELECT} WHERE t.id = ?");
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let Some((
        id,
        employee_id,
        employee_no,
        employee_name,
        period_start,
        period_end,
        worked_minutes,
        expected_minutes,
        balance_minutes,
        overtime_minutes,
        status,
        rejection_reason,
        evaluation_json,
    )) = row
    else {
        return Err(ApiError::not_found("Stundenzettel nicht gefunden"));
    };
    Ok(Json(TimesheetDto::from(TimesheetRow {
        id,
        employee_id,
        employee_no,
        employee_name,
        period_start,
        period_end,
        worked_minutes,
        expected_minutes,
        balance_minutes,
        overtime_minutes,
        status,
        rejection_reason,
        evaluation_json,
    })))
}

#[derive(Deserialize)]
struct RebuildTimesheetsQuery {
    #[serde(default)]
    week_start: Option<String>,
}

async fn rebuild_timesheets(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<RebuildTimesheetsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Update)?;

    let (period_start, period_end) = if let Some(ref ws) = q.week_start {
        let start = DateTime::parse_from_rfc3339(ws)
            .map_err(|_| ApiError::bad_request("week_start ungültig"))?
            .with_timezone(&Utc);
        (start, start + Duration::days(7))
    } else {
        week_bounds_utc(Utc::now())
    };

    let (updated, warnings) = if can_manage_others(&session) {
        rebuild_timesheets_for_week(&state.db, period_start)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
    } else {
        let emp_id = employee_for_user(&state.db, session.user_id)
            .await?
            .to_string();
        rebuild_timesheet_for_employee_week(&state.db, &emp_id, period_start)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
    };

    Ok(Json(serde_json::json!({
        "updated": updated,
        "period_start": period_start.to_rfc3339(),
        "period_end": period_end.to_rfc3339(),
        "warnings": warnings
    })))
}

pub(crate) fn current_week_bounds(now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    week_bounds_utc(now)
}
