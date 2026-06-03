use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, put},
    Json, Router,
};
use sqlx::SqlitePool;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use timeshards_core::permissions::{Action, Resource};
use timeshards_core::ApiError;

use timeshards_db::{rebuild_timesheets_for_calendar, REBUILD_WEEKS_CALENDAR_EDIT};

use crate::auth::{auth_from_headers, require_permission};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/time/work-rotation-plans", get(list_rotation_plans))
        .route(
            "/api/v1/time/work-calendars/{id}/rotation",
            put(set_calendar_rotation),
        )
        .route(
            "/api/v1/time/work-rotation-plans/{id}/slots",
            put(update_rotation_slots),
        )
}

#[derive(Serialize)]
struct RotationSlotDto {
    slot_index: i32,
    workday_model_id: String,
    model_name: String,
}

#[derive(Serialize)]
struct RotationPlanDto {
    id: String,
    name: String,
    anchor_date: String,
    cycle_days: i32,
    slots: Vec<RotationSlotDto>,
}

#[derive(Deserialize)]
struct SetRotationBody {
    rotation_plan_id: Option<String>,
}

#[derive(Serialize)]
struct SetRotationResult {
    calendar_id: String,
    rotation_plan_id: Option<String>,
}

#[derive(Deserialize)]
struct RotationSlotInput {
    slot_index: i32,
    workday_model_id: String,
}

#[derive(Deserialize)]
struct UpdateRotationSlotsBody {
    slots: Vec<RotationSlotInput>,
}

#[derive(Serialize)]
struct UpdateRotationSlotsResult {
    plan_id: String,
    slots_updated: usize,
}

async fn list_rotation_plans(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<RotationPlanDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;

    let plans: Vec<(String, String, String, i32)> = sqlx::query_as(
        "SELECT id, name, anchor_date, cycle_days FROM work_rotation_plans ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut out = Vec::new();
    for (id, name, anchor_date, cycle_days) in plans {
        let slots: Vec<(i32, String, String)> = sqlx::query_as(
            r#"
            SELECT s.slot_index, s.workday_model_id, m.name
            FROM work_rotation_slots s
            JOIN workday_models m ON m.id = s.workday_model_id
            WHERE s.plan_id = ?
            ORDER BY s.slot_index
            "#,
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

        out.push(RotationPlanDto {
            id,
            name,
            anchor_date,
            cycle_days,
            slots: slots
                .into_iter()
                .map(|(slot_index, workday_model_id, model_name)| RotationSlotDto {
                    slot_index,
                    workday_model_id,
                    model_name,
                })
                .collect(),
        });
    }
    Ok(Json(out))
}

async fn set_calendar_rotation(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(calendar_id): Path<String>,
    Json(body): Json<SetRotationBody>,
) -> Result<Json<SetRotationResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    if let Some(pid) = &body.rotation_plan_id {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_rotation_plans WHERE id = ?")
            .bind(pid)
            .fetch_one(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        if exists == 0 {
            return Err(ApiError::bad_request("Umschaltplan nicht gefunden"));
        }
    }

    let cal_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_calendars WHERE id = ?")
        .bind(&calendar_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if cal_exists == 0 {
        return Err(ApiError::bad_request("Arbeitskalender nicht gefunden"));
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE work_calendars SET rotation_plan_id = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&body.rotation_plan_id)
    .bind(&now)
    .bind(&calendar_id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let _ = rebuild_timesheets_for_calendar(&state.db, &calendar_id, REBUILD_WEEKS_CALENDAR_EDIT)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(SetRotationResult {
        calendar_id,
        rotation_plan_id: body.rotation_plan_id,
    }))
}

async fn update_rotation_slots(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
    Json(body): Json<UpdateRotationSlotsBody>,
) -> Result<Json<UpdateRotationSlotsResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Update)?;

    let cycle_days: i32 = sqlx::query_scalar(
        "SELECT cycle_days FROM work_rotation_plans WHERE id = ?",
    )
    .bind(&plan_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::bad_request("Umschaltplan nicht gefunden"))?;

    if body.slots.is_empty() {
        return Err(ApiError::bad_request("Mindestens ein Slot erforderlich"));
    }
    for s in &body.slots {
        if s.slot_index < 0 || s.slot_index >= cycle_days {
            return Err(ApiError::bad_request(format!(
                "slot_index {} außerhalb 0..{}",
                s.slot_index, cycle_days - 1
            )));
        }
        let model_exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM workday_models WHERE id = ?")
                .bind(&s.workday_model_id)
                .fetch_one(&state.db)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        if model_exists == 0 {
            return Err(ApiError::bad_request("Tagesperiode nicht gefunden"));
        }
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    sqlx::query("DELETE FROM work_rotation_slots WHERE plan_id = ?")
        .bind(&plan_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    for s in &body.slots {
        sqlx::query(
            "INSERT INTO work_rotation_slots (plan_id, slot_index, workday_model_id) VALUES (?, ?, ?)",
        )
        .bind(&plan_id)
        .bind(s.slot_index)
        .bind(&s.workday_model_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    tx.commit()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    rebuild_calendars_for_rotation_plan(&state.db, &plan_id).await?;

    Ok(Json(UpdateRotationSlotsResult {
        plan_id,
        slots_updated: body.slots.len(),
    }))
}

async fn rebuild_calendars_for_rotation_plan(
    pool: &SqlitePool,
    plan_id: &str,
) -> Result<(), ApiError> {
    let calendar_ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM work_calendars WHERE rotation_plan_id = ?",
    )
    .bind(plan_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    for cal_id in calendar_ids {
        let _ = rebuild_timesheets_for_calendar(pool, &cal_id, REBUILD_WEEKS_CALENDAR_EDIT)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    Ok(())
}
