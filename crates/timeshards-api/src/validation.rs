use sqlx::SqlitePool;
use timeshards_core::ApiError;

/// RFC3339 interval: end must be strictly after start.
pub fn validate_interval(starts_at: &str, ends_at: &str) -> Result<(), ApiError> {
    if ends_at <= starts_at {
        return Err(ApiError::bad_request(
            "Ende muss nach Beginn liegen",
        ));
    }
    Ok(())
}

/// Overlap with pending or approved absence requests for the same employee.
pub async fn ensure_no_absence_overlap(
    pool: &SqlitePool,
    employee_id: &str,
    starts_at: &str,
    ends_at: &str,
    exclude_id: Option<&str>,
) -> Result<(), ApiError> {
    let mut sql = String::from(
        r#"
        SELECT COUNT(*) FROM absence_requests
        WHERE employee_id = ?
          AND status IN ('pending', 'approved')
          AND starts_at < ?
          AND ends_at > ?
        "#,
    );
    if exclude_id.is_some() {
        sql.push_str(" AND id != ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql)
        .bind(employee_id)
        .bind(ends_at)
        .bind(starts_at);
    if let Some(id) = exclude_id {
        q = q.bind(id);
    }
    let count = q
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if count > 0 {
        return Err(ApiError::bad_request(
            "Zeitraum überschneidet sich mit einem anderen Abwesenheitsantrag",
        ));
    }
    Ok(())
}

pub async fn count_absence_overlap(
    pool: &SqlitePool,
    employee_id: &str,
    starts_at: &str,
    ends_at: &str,
) -> Result<i64, ApiError> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM absence_requests
        WHERE employee_id = ?
          AND status IN ('pending', 'approved')
          AND starts_at < ?
          AND ends_at > ?
        "#,
    )
    .bind(employee_id)
    .bind(ends_at)
    .bind(starts_at)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(count)
}

/// Overlap with planned or published shifts for the same employee.
pub async fn ensure_no_shift_overlap(
    pool: &SqlitePool,
    employee_id: &str,
    starts_at: &str,
    ends_at: &str,
    exclude_id: Option<&str>,
) -> Result<(), ApiError> {
    let mut sql = String::from(
        r#"
        SELECT COUNT(*) FROM shift_instances
        WHERE employee_id = ?
          AND status NOT IN ('cancelled')
          AND starts_at < ?
          AND ends_at > ?
        "#,
    );
    if exclude_id.is_some() {
        sql.push_str(" AND id != ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql)
        .bind(employee_id)
        .bind(ends_at)
        .bind(starts_at);
    if let Some(id) = exclude_id {
        q = q.bind(id);
    }
    let count = q
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if count > 0 {
        return Err(ApiError::bad_request(
            "Zeitraum überschneidet sich mit einer anderen Schicht",
        ));
    }
    Ok(())
}

pub async fn count_shift_overlap(
    pool: &SqlitePool,
    employee_id: &str,
    starts_at: &str,
    ends_at: &str,
) -> Result<i64, ApiError> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM shift_instances
        WHERE employee_id = ?
          AND status NOT IN ('cancelled')
          AND starts_at < ?
          AND ends_at > ?
        "#,
    )
    .bind(employee_id)
    .bind(ends_at)
    .bind(starts_at)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(count)
}
