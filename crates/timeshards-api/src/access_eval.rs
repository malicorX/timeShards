use sqlx::SqlitePool;
use timeshards_core::ApiError;

use crate::schedule::schedule_allows;

pub struct AccessDecision {
    pub decision: &'static str,
    pub reason_code: &'static str,
}

pub async fn evaluate_access(
    pool: &SqlitePool,
    employee_id: &str,
    zone_id: Option<&str>,
    door_id: &str,
    reader_id: &str,
    reader_in_id: Option<&str>,
    reader_out_id: Option<&str>,
) -> Result<AccessDecision, ApiError> {
    if let Some(apb) = check_antipassback(pool, employee_id, door_id, reader_id, reader_in_id).await?
    {
        return Ok(apb);
    }

    let Some(zone_id) = zone_id else {
        return Ok(AccessDecision {
            decision: "grant",
            reason_code: "ok",
        });
    };

    let zone_rules: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM access_rules WHERE zone_id = ?",
    )
    .bind(zone_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    if zone_rules == 0 {
        return Ok(AccessDecision {
            decision: "grant",
            reason_code: "ok",
        });
    }

    let now = chrono::Utc::now();
    let now_str = now.to_rfc3339();
    let rules: Vec<Option<String>> = sqlx::query_scalar(
        r#"
        SELECT schedule_json FROM access_rules
        WHERE principal_type = 'employee'
          AND principal_id = ?
          AND zone_id = ?
          AND (door_id IS NULL OR door_id = ?)
          AND mode = 'allow'
          AND valid_from <= ?
          AND (valid_to IS NULL OR valid_to > ?)
        "#,
    )
    .bind(employee_id)
    .bind(zone_id)
    .bind(door_id)
    .bind(&now_str)
    .bind(&now_str)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    if rules.is_empty() {
        let _ = reader_out_id;
        return Ok(AccessDecision {
            decision: "deny",
            reason_code: "no_permission",
        });
    }

    for sched in &rules {
        if schedule_allows(sched.as_deref(), now) {
            return Ok(AccessDecision {
                decision: "grant",
                reason_code: "ok",
            });
        }
    }

    Ok(AccessDecision {
        decision: "deny",
        reason_code: "schedule_restricted",
    })
}

async fn check_antipassback(
    pool: &SqlitePool,
    employee_id: &str,
    door_id: &str,
    reader_id: &str,
    reader_in_id: Option<&str>,
) -> Result<Option<AccessDecision>, ApiError> {
    let is_entry = reader_in_id.map(|r| r == reader_id).unwrap_or(false);
    if !is_entry {
        return Ok(None);
    }

    let last: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT decision, raw_payload_json FROM access_events
        WHERE employee_id = ? AND door_id = ? AND decision = 'grant'
        ORDER BY occurred_at DESC LIMIT 1
        "#,
    )
    .bind(employee_id)
    .bind(door_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some((_, payload)) = last else {
        return Ok(None);
    };

    let last_reader = serde_json::from_str::<serde_json::Value>(&payload)
        .ok()
        .and_then(|v| v.get("reader_id").and_then(|r| r.as_str().map(str::to_string)));

    if last_reader.as_deref() == Some(reader_id) {
        return Ok(Some(AccessDecision {
            decision: "deny",
            reason_code: "antipassback",
        }));
    }
    Ok(None)
}

pub async fn employee_inside_zone(
    pool: &SqlitePool,
    employee_id: &str,
    zone_id: &str,
) -> Result<bool, ApiError> {
    let last: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT ae.decision, ae.raw_payload_json, d.reader_in_id, d.reader_out_id
        FROM access_events ae
        LEFT JOIN doors d ON d.id = ae.door_id
        WHERE ae.employee_id = ? AND ae.zone_id = ?
        ORDER BY ae.occurred_at DESC
        LIMIT 1
        "#,
    )
    .bind(employee_id)
    .bind(zone_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some((decision, payload, reader_in, reader_out)) = last else {
        return Ok(false);
    };
    if decision != "grant" {
        return Ok(false);
    }
    let reader = serde_json::from_str::<serde_json::Value>(&payload)
        .ok()
        .and_then(|v| v.get("reader_id").and_then(|r| r.as_str().map(str::to_string)));
    let Some(reader_id) = reader else {
        return Ok(true);
    };
    if reader_out.as_deref() == Some(reader_id.as_str()) {
        return Ok(false);
    }
    if reader_in.as_deref() == Some(reader_id.as_str()) {
        return Ok(true);
    }
    Ok(true)
}
