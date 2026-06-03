use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use timeshards_core::permissions::{Action, Resource};
use timeshards_core::ApiError;
use timeshards_db::audit::write_audit;
use timeshards_db::rebuild_timesheets_for_absence_range;
use uuid::Uuid;

use crate::auth::{auth_from_headers, can_manage_others, require_permission};
use crate::routes::time::employee_for_user;
use crate::state::AppState;
use crate::validation::{ensure_no_absence_overlap, validate_interval};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/absences", get(list_absences).post(create_absence))
        .route("/api/v1/absences/{id}/approve", post(approve_absence))
        .route(
            "/api/v1/absences/approve-pending",
            post(approve_pending_absences),
        )
        .route("/api/v1/absences/{id}/reject", post(reject_absence))
        .route("/api/v1/absences/{id}/cancel", post(cancel_absence))
}

#[derive(Serialize)]
struct AbsenceDto {
    id: String,
    employee_id: String,
    employee_no: String,
    employee_name: String,
    absence_type: String,
    starts_at: String,
    ends_at: String,
    status: String,
    reason: Option<String>,
    decision_note: Option<String>,
}

#[derive(Deserialize)]
struct CreateAbsenceBody {
    #[serde(default)]
    employee_id: Option<String>,
    absence_type: String,
    starts_at: String,
    ends_at: String,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
struct AbsenceListQuery {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    employee_id: Option<String>,
}

#[derive(Deserialize)]
struct DecisionBody {
    #[serde(default)]
    note: Option<String>,
}

async fn list_absences(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<AbsenceListQuery>,
) -> Result<Json<Vec<AbsenceDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Absence, Action::Read)?;

    let show_all = can_manage_others(&session);
    let employee_filter = if let Some(eid) = q.employee_id {
        Some(eid)
    } else if !show_all {
        Some(employee_for_user(&state.db, session.user_id).await?.to_string())
    } else {
        None
    };

    let mut sql = String::from(
        r#"
        SELECT a.id, a.employee_id, e.employee_no, e.display_name, a.absence_type,
               a.starts_at, a.ends_at, a.status, a.reason, a.decision_note
        FROM absence_requests a
        JOIN employees e ON e.id = a.employee_id
        WHERE 1=1
        "#,
    );
    if employee_filter.is_some() {
        sql.push_str(" AND a.employee_id = ?");
    }
    if q.status.is_some() {
        sql.push_str(" AND a.status = ?");
    }
    sql.push_str(" ORDER BY a.starts_at DESC LIMIT 100");

    let mut query = sqlx::query_as::<_, (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )>(&sql);

    if let Some(ref eid) = employee_filter {
        query = query.bind(eid);
    }
    if let Some(ref st) = q.status {
        query = query.bind(st);
    }

    let rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    employee_id,
                    employee_no,
                    employee_name,
                    absence_type,
                    starts_at,
                    ends_at,
                    status,
                    reason,
                    decision_note,
                )| AbsenceDto {
                    id,
                    employee_id,
                    employee_no,
                    employee_name,
                    absence_type,
                    starts_at,
                    ends_at,
                    status,
                    reason,
                    decision_note,
                },
            )
            .collect(),
    ))
}

async fn create_absence(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateAbsenceBody>,
) -> Result<Json<AbsenceDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Absence, Action::Create)?;

    let employee_id = if let Some(eid) = body.employee_id {
        if !can_manage_others(&session) {
            return Err(ApiError::forbidden(
                "Nur eigene Abwesenheiten anlegbar",
            ));
        }
        eid
    } else {
        employee_for_user(&state.db, session.user_id)
            .await?
            .to_string()
    };

    if !matches!(
        body.absence_type.as_str(),
        "urlaub" | "krank" | "sonder" | "vacation" | "sick" | "special"
    ) {
        return Err(ApiError::bad_request(
            "Typ: urlaub, krank oder sonder",
        ));
    }

    validate_interval(&body.starts_at, &body.ends_at)?;
    ensure_no_absence_overlap(
        &state.db,
        &employee_id,
        &body.starts_at,
        &body.ends_at,
        None,
    )
    .await?;

    let id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO absence_requests (
            id, employee_id, absence_type, starts_at, ends_at, status, reason, created_at
        ) VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(&employee_id)
    .bind(&body.absence_type)
    .bind(&body.starts_at)
    .bind(&body.ends_at)
    .bind(&body.reason)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "create",
        "absence_request",
        Some(id),
        None,
        None,
        Some(serde_json::json!({ "type": body.absence_type })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    fetch_absence(&state.db, &id.to_string()).await
}

async fn load_absence(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> Result<(String, String), ApiError> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT employee_id, status FROM absence_requests WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    row.ok_or_else(|| ApiError::not_found("Abwesenheit nicht gefunden"))
}

async fn load_absence_interval(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> Result<(String, String, String), ApiError> {
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT employee_id, starts_at, ends_at FROM absence_requests WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    row.ok_or_else(|| ApiError::not_found("Abwesenheit nicht gefunden"))
}

#[derive(Serialize)]
struct BulkApproveResult {
    approved: u64,
}

async fn approve_pending_absences(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<BulkApproveResult>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Absence, Action::Approve)?;

    let pending: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT employee_id, starts_at, ends_at FROM absence_requests WHERE status = 'pending'",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        UPDATE absence_requests
        SET status = 'approved', decided_by = ?, decided_at = ?
        WHERE status = 'pending'
        "#,
    )
    .bind(session.user_id.to_string())
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    for (employee_id, starts_at, ends_at) in pending {
        let _ = rebuild_timesheets_for_absence_range(&state.db, &employee_id, &starts_at, &ends_at)
            .await;
    }

    Ok(Json(BulkApproveResult {
        approved: result.rows_affected(),
    }))
}

async fn approve_absence(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<DecisionBody>,
) -> Result<Json<AbsenceDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Absence, Action::Approve)?;
    let (_, status) = load_absence(&state.db, &id).await?;
    if status != "pending" {
        return Err(ApiError::bad_request("Nur offene Anträge können freigegeben werden"));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE absence_requests SET status = 'approved', decided_by = ?, decided_at = ?, decision_note = ?
        WHERE id = ?
        "#,
    )
    .bind(session.user_id.to_string())
    .bind(&now)
    .bind(&body.note)
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    if let Ok((employee_id, starts_at, ends_at)) = load_absence_interval(&state.db, &id).await {
        let _ = rebuild_timesheets_for_absence_range(&state.db, &employee_id, &starts_at, &ends_at)
            .await;
    }

    fetch_absence(&state.db, &id).await
}

async fn reject_absence(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<DecisionBody>,
) -> Result<Json<AbsenceDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Absence, Action::Approve)?;
    let (_, status) = load_absence(&state.db, &id).await?;
    if status != "pending" {
        return Err(ApiError::bad_request("Nur offene Anträge können abgelehnt werden"));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE absence_requests SET status = 'rejected', decided_by = ?, decided_at = ?, decision_note = ?
        WHERE id = ?
        "#,
    )
    .bind(session.user_id.to_string())
    .bind(&now)
    .bind(&body.note)
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    fetch_absence(&state.db, &id).await
}

async fn cancel_absence(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<AbsenceDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    let (employee_id, status) = load_absence(&state.db, &id).await?;
    if !matches!(status.as_str(), "pending" | "approved") {
        return Err(ApiError::bad_request("Antrag kann nicht mehr storniert werden"));
    }
    let own = employee_for_user(&state.db, session.user_id).await.ok();
    let is_own = own.map(|e| e.to_string()) == Some(employee_id);
    if is_own {
        require_permission(&session, Resource::Absence, Action::Create)?;
    } else {
        require_permission(&session, Resource::Absence, Action::Approve)?;
    }
    sqlx::query("UPDATE absence_requests SET status = 'cancelled' WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    fetch_absence(&state.db, &id).await
}

async fn fetch_absence(pool: &sqlx::SqlitePool, id: &str) -> Result<Json<AbsenceDto>, ApiError> {
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT a.id, a.employee_id, e.employee_no, e.display_name, a.absence_type,
               a.starts_at, a.ends_at, a.status, a.reason, a.decision_note
        FROM absence_requests a
        JOIN employees e ON e.id = a.employee_id
        WHERE a.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some(row) = row else {
        return Err(ApiError::not_found("Abwesenheit nicht gefunden"));
    };

    Ok(Json(AbsenceDto {
        id: row.0,
        employee_id: row.1,
        employee_no: row.2,
        employee_name: row.3,
        absence_type: row.4,
        starts_at: row.5,
        ends_at: row.6,
        status: row.7,
        reason: row.8,
        decision_note: row.9,
    }))
}
