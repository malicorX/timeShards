use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use timeshards_db::{
    rebuild_timesheet_for_employee_week, resolve_employee_calendar, week_bounds_utc,
    WeekEvaluationMeta,
};
use std::sync::Arc;
use timeshards_core::{
    events::topics,
    permissions::{Action, Resource},
    ApiError, DomainEvent, EventActor, LoginRequest, LoginResponse,
};
use uuid::Uuid;

use crate::auth::{auth_from_headers, authenticate, can_manage_others, create_session, prune_expired_sessions};
use crate::routes::time::employee_for_user;
use crate::state::AppState;
use timeshards_db::{audit::write_audit, hash_password, is_default_password_login_blocked};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/me/work-summary", get(work_summary))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/change-password", post(change_password))
}

#[derive(Serialize)]
struct WorkSummaryDto {
    pending_timesheets: Option<i64>,
    pending_absences: Option<i64>,
    /// Own absence requests awaiting approval (any employee with a linked profile).
    my_pending_absences: Option<i64>,
    draft_timesheets: Option<i64>,
    team_draft_timesheets: Option<i64>,
    employee_id: Option<String>,
    employee_no: Option<String>,
    is_clocked_in: bool,
    is_on_break: bool,
    /// Cumulative flex account (Gleitzeit) after approved timesheets.
    flex_balance_minutes: Option<i64>,
    /// Draft/pending/approved timesheet for the calendar week containing today (after rebuild).
    current_week: Option<CurrentWeekTimesheetDto>,
    /// Whether an active work-calendar assignment exists for today (linked employee only).
    work_calendar_assigned: Option<bool>,
}

#[derive(Serialize)]
struct CurrentWeekTimesheetDto {
    period_start: String,
    status: String,
    worked_minutes: i64,
    expected_minutes: i64,
    balance_minutes: i64,
    work_calendar_name: Option<String>,
}

async fn user_summary_with_employee(
    pool: &sqlx::SqlitePool,
    session: &timeshards_core::AuthSession,
) -> Result<timeshards_core::UserSummary, ApiError> {
    let mut summary = session.to_summary();
    if let Ok(emp_uuid) = employee_for_user(pool, session.user_id).await {
        let emp_id = emp_uuid.to_string();
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT employee_no, display_name FROM employees WHERE id = ?",
        )
        .bind(&emp_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        summary.employee_id = Some(emp_id);
        if let Some((no, _)) = row {
            summary.employee_no = Some(no);
        }
    }
    Ok(summary)
}

#[derive(Deserialize)]
struct ChangePasswordBody {
    current_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;

    if body.new_password.len() < 6 {
        return Err(ApiError::bad_request("Neues Passwort min. 6 Zeichen"));
    }

    let username = session.username.clone();
    let user_id = authenticate(&state.db, &username, &body.current_password)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::bad_request("Aktuelles Passwort ist falsch"))?;

    if user_id != session.user_id {
        return Err(ApiError::internal("Benutzer-ID stimmt nicht überein"));
    }

    let hash = hash_password(&body.new_password).map_err(|e| ApiError::internal(e.to_string()))?;
    let now = Utc::now().to_rfc3339();

    sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?")
        .bind(&hash)
        .bind(&now)
        .bind(session.user_id.to_string())
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "change_password",
        "user",
        Some(session.user_id),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    if is_default_password_login_blocked(&body.username, &body.password) {
        return Err(ApiError::forbidden(
            "Anmeldung mit Standardpasswort ist deaktiviert. \
             Passwort ändern, Admin-Zurücksetzen, oder TIMESHARDS_DISABLE_DEMO / \
             TIMESHARDS_BLOCK_DEFAULT_PASSWORDS prüfen.",
        ));
    }

    let user_id = authenticate(&state.db, &body.username, &body.password)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::unauthorized("Benutzername oder Passwort ungültig"))?;

    prune_expired_sessions(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let (token, expires_at_str) = create_session(&state.db, user_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let session = crate::auth::resolve_session(&state.db, &token)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::internal("Sitzung konnte nicht geladen werden"))?;

    let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at_str)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .with_timezone(&Utc);

    let event = DomainEvent {
        id: Uuid::new_v4(),
        topic: topics::USER_LOGIN.to_string(),
        schema_version: 1,
        occurred_at: Utc::now(),
        producer: "api.auth".into(),
        correlation_id: None,
        actor: Some(EventActor::User {
            id: user_id,
        }),
        payload: serde_json::json!({ "username": body.username }),
    };
    persist_domain_event(&state, &event).await?;

    write_audit(
        &state.db,
        "user",
        Some(user_id),
        "login",
        "session",
        None,
        None,
        None,
        Some(serde_json::json!({ "username": body.username })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let user = user_summary_with_employee(&state.db, &session).await?;

    Ok(Json(LoginResponse {
        token,
        expires_at,
        user,
    }))
}

async fn me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<timeshards_core::UserSummary>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    Ok(Json(user_summary_with_employee(&state.db, &session).await?))
}

async fn work_summary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<WorkSummaryDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;

    let pending_timesheets = if session.permissions.allows(Resource::Timesheet, Action::Approve) {
        Some(
            sqlx::query_scalar("SELECT COUNT(*) FROM timesheets WHERE status = 'pending'")
                .fetch_one(&state.db)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?,
        )
    } else {
        None
    };

    let pending_absences = if session.permissions.allows(Resource::Absence, Action::Approve) {
        Some(
            sqlx::query_scalar("SELECT COUNT(*) FROM absence_requests WHERE status = 'pending'")
                .fetch_one(&state.db)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?,
        )
    } else {
        None
    };

    let mut draft_timesheets: Option<i64> = None;
    let mut my_pending_absences: Option<i64> = None;
    let mut employee_id: Option<String> = None;
    let mut employee_no: Option<String> = None;
    let mut is_clocked_in = false;
    let mut is_on_break = false;
    let mut flex_balance_minutes: Option<i64> = None;
    let mut current_week: Option<CurrentWeekTimesheetDto> = None;
    let mut work_calendar_assigned: Option<bool> = None;

    let team_draft_timesheets = if can_manage_others(&session) {
        Some(
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM timesheets WHERE status IN ('draft', 'rejected')",
            )
            .fetch_one(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?,
        )
    } else {
        None
    };

    if let Ok(emp_uuid) = employee_for_user(&state.db, session.user_id).await {
        let emp_id = emp_uuid.to_string();
        employee_id = Some(emp_id.clone());
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT employee_no FROM employees WHERE id = ?",
        )
        .bind(&emp_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        employee_no = row.map(|(no,)| no);

        draft_timesheets = Some(
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM timesheets WHERE employee_id = ? AND status IN ('draft', 'rejected')",
            )
            .bind(&emp_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?,
        );

        my_pending_absences = Some(
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM absence_requests WHERE employee_id = ? AND status = 'pending'",
            )
            .bind(&emp_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?,
        );

        let last_kind: Option<String> = sqlx::query_scalar(
            r#"
            SELECT kind FROM time_events
            WHERE employee_id = ?
            ORDER BY occurred_at DESC
            LIMIT 1
            "#,
        )
        .bind(emp_id.to_string())
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

        is_clocked_in = matches!(
            last_kind.as_deref(),
            Some("clock_in") | Some("break_start") | Some("break_end")
        );
        is_on_break = last_kind.as_deref() == Some("break_start");

        flex_balance_minutes = sqlx::query_scalar(
            "SELECT balance_minutes FROM time_accounts WHERE employee_id = ? AND account_kind = 'flex'",
        )
        .bind(emp_id.to_string())
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

        let today = Utc::now().date_naive();
        let has_calendar = resolve_employee_calendar(&state.db, &emp_id, today)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .is_some();
        work_calendar_assigned = Some(has_calendar);

        let (week_start, _) = week_bounds_utc(Utc::now());
        let period_start = week_start.to_rfc3339();

        if has_calendar {
            let exists: Option<String> = sqlx::query_scalar(
                "SELECT id FROM timesheets WHERE employee_id = ? AND period_start = ?",
            )
            .bind(&emp_id)
            .bind(&period_start)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
            if exists.is_none() {
                let _ = rebuild_timesheet_for_employee_week(&state.db, &emp_id, week_start)
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?;
            }
        }

        let row: Option<(i64, i64, i64, String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT worked_minutes, expected_minutes, balance_minutes, status, evaluation_json, period_start
            FROM timesheets
            WHERE employee_id = ? AND period_start = ?
            "#,
        )
        .bind(&emp_id)
        .bind(&period_start)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

        current_week = row.map(
            |(worked, expected, balance, status, evaluation_json, ps)| {
                let work_calendar_name = serde_json::from_str::<WeekEvaluationMeta>(&evaluation_json)
                    .ok()
                    .map(|m| m.work_calendar_name);
                CurrentWeekTimesheetDto {
                    period_start: ps.unwrap_or(period_start),
                    status,
                    worked_minutes: worked,
                    expected_minutes: expected,
                    balance_minutes: balance,
                    work_calendar_name,
                }
            },
        );
    }

    Ok(Json(WorkSummaryDto {
        pending_timesheets,
        pending_absences,
        my_pending_absences,
        draft_timesheets,
        team_draft_timesheets,
        employee_id,
        employee_no,
        is_clocked_in,
        is_on_break,
        flex_balance_minutes,
        current_week,
        work_calendar_assigned,
    }))
}

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    let hash = crate::auth::hash_token(auth);
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(hash)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "logout",
        "session",
        None,
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn persist_domain_event(state: &AppState, event: &DomainEvent) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO domain_events (id, topic, schema_version, occurred_at, producer, correlation_id, payload_json)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(event.id.to_string())
    .bind(&event.topic)
    .bind(event.schema_version as i64)
    .bind(event.occurred_at.to_rfc3339())
    .bind(&event.producer)
    .bind(event.correlation_id.map(|u| u.to_string()))
    .bind(event.payload.to_string())
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}
