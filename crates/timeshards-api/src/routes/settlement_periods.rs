use axum::{
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use timeshards_core::permissions::{Action, Resource};
use timeshards_core::ApiError;
use timeshards_db::{close_month, list_closed_periods, preview_month, SettlementPeriodRow};

use crate::auth::{auth_from_headers, require_permission};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/time/settlement-periods/preview",
            get(preview_settlement),
        )
        .route(
            "/api/v1/time/settlement-periods",
            get(list_periods).post(close_settlement),
        )
}

#[derive(Deserialize)]
struct PeriodQuery {
    year: i32,
    month: u32,
    employee_id: String,
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    year: Option<i32>,
    #[serde(default)]
    month: Option<u32>,
    #[serde(default)]
    employee_id: Option<String>,
}

#[derive(Deserialize)]
struct CloseBody {
    year: i32,
    month: u32,
    employee_id: String,
}

#[derive(Serialize)]
struct PeriodDto {
    id: String,
    employee_id: String,
    year: i32,
    month: u32,
    status: String,
    worked_minutes: i64,
    expected_minutes: i64,
    balance_minutes: i64,
    overtime_minutes: i64,
    weeks_count: i32,
    closed_at: Option<String>,
}

impl From<SettlementPeriodRow> for PeriodDto {
    fn from(r: SettlementPeriodRow) -> Self {
        Self {
            id: r.id,
            employee_id: r.employee_id,
            year: r.year,
            month: r.month,
            status: r.status,
            worked_minutes: r.worked_minutes,
            expected_minutes: r.expected_minutes,
            balance_minutes: r.balance_minutes,
            overtime_minutes: r.overtime_minutes,
            weeks_count: r.weeks_count,
            closed_at: r.closed_at,
        }
    }
}

async fn preview_settlement(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<PeriodQuery>,
) -> Result<Json<timeshards_db::MonthSettlementPreview>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Read)?;
    if q.month < 1 || q.month > 12 {
        return Err(ApiError::bad_request("Monat muss 1–12 sein"));
    }
    let preview = preview_month(&state.db, &q.employee_id, q.year, q.month)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(preview))
}

async fn list_periods(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<PeriodDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Read)?;
    let rows = list_closed_periods(
        &state.db,
        q.year,
        q.month,
        q.employee_id.as_deref(),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(rows.into_iter().map(PeriodDto::from).collect()))
}

async fn close_settlement(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CloseBody>,
) -> Result<Json<PeriodDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Approve)?;
    if body.month < 1 || body.month > 12 {
        return Err(ApiError::bad_request("Monat muss 1–12 sein"));
    }
    let closed_by = session.user_id.to_string();
    let row = close_month(
        &state.db,
        &body.employee_id,
        body.year,
        body.month,
        Some(&closed_by),
    )
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("bereits") || msg.contains("nicht abschließbar") || msg.contains("Keine freigegebenen") {
            ApiError::bad_request(msg)
        } else {
            ApiError::internal(msg)
        }
    })?;
    Ok(Json(PeriodDto::from(row)))
}
