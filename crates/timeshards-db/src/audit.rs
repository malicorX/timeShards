use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn write_audit(
    pool: &SqlitePool,
    actor_type: &str,
    actor_id: Option<Uuid>,
    action: &str,
    object_type: &str,
    object_id: Option<Uuid>,
    reason: Option<&str>,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    let id = Uuid::new_v4();
    let occurred_at = Utc::now().to_rfc3339();
    let hash_prev: Option<String> = sqlx::query_scalar(
        "SELECT hash_self FROM audit_log ORDER BY occurred_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    let payload = format!(
        "{}{}{}{}{}",
        id,
        action,
        object_type,
        object_id.map(|u| u.to_string()).unwrap_or_default(),
        occurred_at
    );
    let mut hasher = Sha256::new();
    if let Some(prev) = &hash_prev {
        hasher.update(prev.as_bytes());
    }
    hasher.update(payload.as_bytes());
    let hash_self = hex::encode(hasher.finalize());

    sqlx::query(
        r#"
        INSERT INTO audit_log (
            id, actor_type, actor_id, action, object_type, object_id,
            occurred_at, reason, before_json, after_json, hash_prev, hash_self
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(actor_type)
    .bind(actor_id.map(|u| u.to_string()))
    .bind(action)
    .bind(object_type)
    .bind(object_id.map(|u| u.to_string()))
    .bind(&occurred_at)
    .bind(reason)
    .bind(before.map(|v| v.to_string()))
    .bind(after.map(|v| v.to_string()))
    .bind(hash_prev)
    .bind(&hash_self)
    .execute(pool)
    .await?;

    Ok(())
}
