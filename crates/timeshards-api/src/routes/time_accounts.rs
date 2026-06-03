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
use timeshards_db::list_account_balances;

use crate::auth::{auth_from_headers, can_manage_others, require_permission};
use crate::routes::time::employee_for_user;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/time/accounts", get(list_accounts))
}

#[derive(Serialize)]
struct TimeAccountDto {
    account_kind: String,
    label: String,
    balance_minutes: i64,
}

#[derive(Deserialize)]
struct AccountsQuery {
    #[serde(default)]
    employee_id: Option<String>,
}

fn account_label(kind: &str) -> &'static str {
    match kind {
        "flex" => "Gleitzeit / Wochensaldo",
        "overtime" => "Überstundenkonto",
        _ => "Konto",
    }
}

async fn list_accounts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<AccountsQuery>,
) -> Result<Json<Vec<TimeAccountDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Timesheet, Action::Read)?;

    let employee_id = if can_manage_others(&session) {
        if let Some(eid) = q.employee_id {
            eid
        } else {
            employee_for_user(&state.db, session.user_id)
                .await?
                .to_string()
        }
    } else {
        employee_for_user(&state.db, session.user_id)
            .await?
            .to_string()
    };

    let rows = list_account_balances(&state.db, &employee_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(account_kind, balance_minutes)| TimeAccountDto {
                label: account_label(&account_kind).to_string(),
                account_kind,
                balance_minutes,
            })
            .collect(),
    ))
}
