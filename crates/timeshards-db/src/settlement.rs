//! Settlement rules (Wochen-/Monatsperiode) — weekly warnings and future account close-out.

use sqlx::SqlitePool;

use crate::work_model::SettlementRuleConfig;

pub async fn load_settlement_config(
    pool: &SqlitePool,
    rule_id: &str,
) -> anyhow::Result<SettlementRuleConfig> {
    let json: Option<String> =
        sqlx::query_scalar("SELECT config_json FROM settlement_rules WHERE id = ?")
            .bind(rule_id)
            .fetch_optional(pool)
            .await?;
    match json {
        Some(j) => Ok(serde_json::from_str(&j).unwrap_or_default()),
        None => Ok(SettlementRuleConfig::default()),
    }
}
