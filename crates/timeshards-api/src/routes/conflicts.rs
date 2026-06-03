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

use crate::auth::{auth_from_headers, can_manage_others, require_permission};
use crate::routes::time::employee_for_user;
use crate::state::AppState;
use crate::validation::{count_absence_overlap, count_shift_overlap, validate_interval};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/absences/conflicts", get(check_absence_conflicts))
        .route("/api/v1/time/shifts/conflicts", get(check_shift_conflicts))
}

#[derive(Serialize)]
struct ConflictCheckDto {
    has_conflict: bool,
    count: i64,
    message: Option<String>,
}

#[derive(Deserialize)]
struct ConflictQuery {
    starts_at: String,
    ends_at: String,
    #[serde(default)]
    employee_id: Option<String>,
}

async fn resolve_employee_id(
    state: &AppState,
    session: &timeshards_core::AuthSession,
    q: &ConflictQuery,
) -> Result<String, ApiError> {
    if let Some(eid) = &q.employee_id {
        if !can_manage_others(session) {
            let own = employee_for_user(&state.db, session.user_id).await?;
            if own.to_string() != *eid {
                return Err(ApiError::forbidden(
                    "Nur eigene Zeiträume prüfbar",
                ));
            }
        }
        Ok(eid.clone())
    } else {
        Ok(employee_for_user(&state.db, session.user_id)
            .await?
            .to_string())
    }
}

async fn check_absence_conflicts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ConflictQuery>,
) -> Result<Json<ConflictCheckDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Absence, Action::Read)?;
    validate_interval(&q.starts_at, &q.ends_at)?;
    let employee_id = resolve_employee_id(&state, &session, &q).await?;
    let count = count_absence_overlap(&state.db, &employee_id, &q.starts_at, &q.ends_at).await?;
    Ok(Json(conflict_dto(count, "Abwesenheitsantrag")))
}

async fn check_shift_conflicts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ConflictQuery>,
) -> Result<Json<ConflictCheckDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Shift, Action::Read)?;
    validate_interval(&q.starts_at, &q.ends_at)?;
    let employee_id = if let Some(eid) = &q.employee_id {
        if !can_manage_others(&session) {
            let own = employee_for_user(&state.db, session.user_id).await?;
            if own.to_string() != *eid {
                return Err(ApiError::forbidden(
                    "Nur eigene Zeiträume prüfbar",
                ));
            }
        }
        eid.clone()
    } else if can_manage_others(&session) {
        return Err(ApiError::bad_request(
            "employee_id erforderlich für Schicht-Konfliktprüfung",
        ));
    } else {
        employee_for_user(&state.db, session.user_id)
            .await?
            .to_string()
    };
    let count = count_shift_overlap(&state.db, &employee_id, &q.starts_at, &q.ends_at).await?;
    Ok(Json(conflict_dto(count, "Schicht")))
}

fn conflict_dto(count: i64, kind: &str) -> ConflictCheckDto {
    ConflictCheckDto {
        has_conflict: count > 0,
        count,
        message: if count > 0 {
            Some(format!(
                "{count} überschneidende {kind}(en) im gewählten Zeitraum"
            ))
        } else {
            None
        },
    }
}
