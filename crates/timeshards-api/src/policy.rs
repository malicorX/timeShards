//! Policy rules (implemented in timeshards-db).
use sqlx::SqlitePool;
use timeshards_core::ApiError;

pub use timeshards_db::policy::DePolicyRules;

pub async fn load_active_policy(pool: &SqlitePool) -> Result<DePolicyRules, ApiError> {
    timeshards_db::load_active_policy(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))
}
