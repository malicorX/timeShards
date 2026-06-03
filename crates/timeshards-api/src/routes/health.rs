use axum::{
    extract::State,
    http::{header, HeaderValue},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;
use timeshards_db::{
    count_current_week_drafts_without_soll, is_block_default_passwords_enabled,
    is_demo_seeding_enabled,
};
use timeshards_hardware::{
    hardware_adapter_active, hardware_adapter_configured, hardware_tcp_listen_addr,
};

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: &'static str,
    pub version: &'static str,
    pub database: String,
    /// Demo users (`demo`/`manager`) and sample week data are seeded on startup when true.
    pub demo_seeding_enabled: bool,
    /// Built-in passwords (`admin`/`admin`, etc.) are rejected at login when true.
    pub default_password_login_blocked: bool,
    /// Active adapter after bootstrap (`sim` or `external`).
    pub hardware_adapter: &'static str,
    /// Set when `TIMESHARDS_HW_ADAPTER` is invalid but the server fell back to `sim`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_adapter_configured: Option<String>,
    /// Set when external adapter listens for TCP credential lines (`TIMESHARDS_HW_TCP_ADDR`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_tcp_listen: Option<String>,
    /// Work-calendar foundation snapshot (no auth).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_foundation: Option<TimeFoundationHealth>,
}

#[derive(Serialize)]
pub struct TimeFoundationHealth {
    pub workday_models: i64,
    pub work_calendars: i64,
    pub active_employees: i64,
    pub employees_without_work_calendar: i64,
    /// Draft/rejected current KW without Soll (or no row) while calendar is assigned.
    pub current_week_drafts_without_soll: i64,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/openapi.json", get(openapi_spec))
}

async fn openapi_spec() -> impl IntoResponse {
    static SPEC: &str = include_str!("../../../../docs/openapi.json");
    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        SPEC,
    )
}

async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let database = match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => "ok",
        Err(_) => "error",
    };
    let status = if database == "ok" { "ok" } else { "degraded" };

    let time_foundation = if database == "ok" {
        load_time_foundation_health(&state.db).await.ok()
    } else {
        None
    };

    Json(HealthResponse {
        status: status.into(),
        service: "timeshards-server",
        version: env!("CARGO_PKG_VERSION"),
        database: database.into(),
        demo_seeding_enabled: is_demo_seeding_enabled(),
        default_password_login_blocked: !is_demo_seeding_enabled()
            || is_block_default_passwords_enabled(),
        hardware_adapter: hardware_adapter_active(),
        hardware_adapter_configured: {
            let configured = hardware_adapter_configured();
            if configured == "unknown" || configured != hardware_adapter_active() {
                Some(configured.to_string())
            } else {
                None
            }
        },
        hardware_tcp_listen: hardware_tcp_listen_addr(),
        time_foundation,
    })
}

async fn load_time_foundation_health(
    pool: &sqlx::SqlitePool,
) -> Result<TimeFoundationHealth, sqlx::Error> {
    let workday_models: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workday_models").fetch_one(pool).await?;
    let work_calendars: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM work_calendars").fetch_one(pool).await?;
    let active_employees: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM employees WHERE active_to IS NULL")
            .fetch_one(pool)
            .await?;
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let employees_without_work_calendar: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM employees e
        WHERE e.active_to IS NULL
          AND NOT EXISTS (
            SELECT 1 FROM employee_work_assignments a
            WHERE a.employee_id = e.id
              AND a.valid_from <= ?
              AND (a.valid_to IS NULL OR substr(a.valid_to, 1, 10) > ?)
          )
        "#,
    )
    .bind(&today)
    .bind(&today)
    .fetch_one(pool)
    .await?;

    let current_week_drafts_without_soll =
        count_current_week_drafts_without_soll(pool).await.unwrap_or(0);

    Ok(TimeFoundationHealth {
        workday_models,
        work_calendars,
        active_employees,
        employees_without_work_calendar,
        current_week_drafts_without_soll,
    })
}
