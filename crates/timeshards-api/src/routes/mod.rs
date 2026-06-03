pub mod absence;
pub mod access;
pub mod admin;
pub mod auth_routes;
pub mod conflicts;
pub mod health;
pub mod reports;
pub mod settlement_periods;
pub mod shift_templates;
pub mod time;
pub mod time_accounts;
pub mod work_calendars;
pub mod work_rotation;

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

pub fn api_router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(health::routes())
        .merge(auth_routes::routes())
        .merge(admin::routes())
        .merge(time::routes())
        .merge(time_accounts::routes())
        .merge(shift_templates::routes())
        .merge(work_calendars::routes())
        .merge(work_rotation::routes())
        .merge(settlement_periods::routes())
        .merge(absence::routes())
        .merge(conflicts::routes())
        .merge(reports::routes())
        .merge(access::routes())
        .with_state(state)
}
