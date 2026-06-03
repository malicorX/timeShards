use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use timeshards_core::permissions::{Action, Resource};
use timeshards_core::ApiError;

use crate::auth::{auth_from_headers, require_permission};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/time/holiday-calendars", get(list_holiday_calendars))
        .route(
            "/api/v1/time/holiday-calendars/{id}/days",
            get(list_holiday_days),
        )
}

#[derive(Serialize)]
struct HolidayCalendarDto {
    id: String,
    name: String,
    region_code: Option<String>,
    year_from: i32,
    year_to: i32,
}

#[derive(Serialize)]
struct HolidayDayDto {
    date: String,
    day_kind: String,
    name: Option<String>,
    workday_model_id: Option<String>,
    model_name: Option<String>,
}

#[derive(Deserialize)]
struct HolidayDaysQuery {
    from: String,
    to: String,
}

async fn list_holiday_calendars(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<HolidayCalendarDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let rows: Vec<(String, String, Option<String>, i32, i32)> = sqlx::query_as(
        "SELECT id, name, region_code, year_from, year_to FROM holiday_calendars ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, region_code, year_from, year_to)| HolidayCalendarDto {
                id,
                name,
                region_code,
                year_from,
                year_to,
            })
            .collect(),
    ))
}

async fn list_holiday_days(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(calendar_id): Path<String>,
    Query(q): Query<HolidayDaysQuery>,
) -> Result<Json<Vec<HolidayDayDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM holiday_calendars WHERE id = ?")
        .bind(&calendar_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if exists == 0 {
        return Err(ApiError::bad_request("Feiertagskalender nicht gefunden"));
    }

    let rows: Vec<(String, String, Option<String>, Option<String>, Option<String>)> =
        sqlx::query_as(
            r#"
            SELECT h.date, h.day_kind, h.name, h.workday_model_id, m.name
            FROM holiday_calendar_days h
            LEFT JOIN workday_models m ON m.id = h.workday_model_id
            WHERE h.calendar_id = ? AND h.date >= ? AND h.date <= ?
            ORDER BY h.date
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
            .map(
                |(date, day_kind, name, workday_model_id, model_name)| HolidayDayDto {
                    date,
                    day_kind,
                    name,
                    workday_model_id,
                    model_name,
                },
            )
            .collect(),
    ))
}
