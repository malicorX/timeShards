use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use timeshards_hardware::{HardwareEvent, RawCredentialPresentation};
use timeshards_core::{
    events::topics,
    permissions::{Action, Resource},
    ApiError, DomainEvent,
};
use uuid::Uuid;

use crate::access_eval::{employee_inside_zone, evaluate_access};
use crate::auth::{auth_from_headers, can_view_all_access_events, require_permission};
use crate::routes::time::employee_for_user;
use crate::state::AppState;
use timeshards_db::audit::write_audit;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/access/zones", get(list_zones).post(create_zone))
        .route("/api/v1/access/doors", get(list_doors).post(create_door))
        .route("/api/v1/access/doors/{id}/status", post(update_door_status))
        .route("/api/v1/access/events", get(list_access_events))
        .route("/api/v1/access/simulate-scan", post(simulate_scan))
        .route("/api/v1/access/hardware-present", post(hardware_present))
        .route("/api/v1/access/badges", get(list_badges).post(create_badge))
        .route("/api/v1/access/badges/{id}/revoke", post(revoke_badge))
        .route(
            "/api/v1/access/rules",
            get(list_access_rules).post(create_access_rule),
        )
        .route(
            "/api/v1/access/rules/{id}",
            patch(update_access_rule).delete(delete_access_rule),
        )
        .route("/api/v1/access/occupancy", get(zone_occupancy))
        .route("/api/v1/access/me", get(my_access_summary))
        .route("/api/v1/access/me/simulate-scan", post(my_simulate_scan))
}

#[derive(Serialize)]
struct ZoneDto {
    id: String,
    name: String,
    site_id: String,
    risk_level: String,
}

#[derive(Serialize)]
struct DoorDto {
    id: String,
    name: String,
    zone_id: Option<String>,
    status: String,
    reader_in_id: Option<String>,
    reader_out_id: Option<String>,
}

#[derive(Deserialize)]
struct SimulateScanRequest {
    credential_uid: String,
    #[serde(default = "default_reader")]
    reader_id: String,
}

fn default_reader() -> String {
    "sim.reader.main".into()
}

async fn list_zones(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<ZoneDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Zone, Action::Read)?;

    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, name, site_id, risk_level FROM zones ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, site_id, risk_level)| ZoneDto {
                id,
                name,
                site_id,
                risk_level,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct UpdateDoorStatusBody {
    status: String,
}

async fn update_door_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateDoorStatusBody>,
) -> Result<Json<DoorDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Door, Action::Update)?;

    if !matches!(
        body.status.as_str(),
        "closed" | "open" | "forced_open" | "alarm"
    ) {
        return Err(ApiError::bad_request(
            "status: closed, open, forced_open, alarm",
        ));
    }

    let updated = sqlx::query("UPDATE doors SET status = ? WHERE id = ?")
        .bind(&body.status)
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::not_found("Tür nicht gefunden"));
    }

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "update",
        "door",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        Some(serde_json::json!({ "status": body.status })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let row: Option<(
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT id, name, zone_id, status, reader_in_id, reader_out_id FROM doors WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some((id, name, zone_id, status, reader_in_id, reader_out_id)) = row else {
        return Err(ApiError::not_found("Tür nicht gefunden"));
    };
    Ok(Json(DoorDto {
        id,
        name,
        zone_id,
        status,
        reader_in_id,
        reader_out_id,
    }))
}

async fn list_doors(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<DoorDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Door, Action::Read)?;

    let rows: Vec<(String, String, Option<String>, String, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT id, name, zone_id, status, reader_in_id, reader_out_id FROM doors ORDER BY name",
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, zone_id, status, reader_in_id, reader_out_id)| DoorDto {
                id,
                name,
                zone_id,
                status,
                reader_in_id,
                reader_out_id,
            })
            .collect(),
    ))
}

#[derive(Serialize)]
struct AccessEventDto {
    id: String,
    occurred_at: String,
    decision: String,
    reason_code: Option<String>,
    employee_no: Option<String>,
    employee_name: Option<String>,
    zone_name: Option<String>,
    door_name: Option<String>,
}

#[derive(Deserialize)]
struct AccessEventsQuery {
    #[serde(default)]
    decision: Option<String>,
    /// RFC3339 — return only events strictly after this timestamp (for polling after hardware-present).
    #[serde(default)]
    since: Option<String>,
    /// Filter by employee personnel number (admins / security only).
    #[serde(default)]
    employee_no: Option<String>,
    #[serde(default = "default_events_limit")]
    limit: u32,
}

fn default_events_limit() -> u32 {
    100
}

async fn list_access_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<AccessEventsQuery>,
) -> Result<Json<Vec<AccessEventDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::AccessEvent, Action::Read)?;

    let limit = q.limit.clamp(1, 500);
    let mut sql = String::from(
        r#"
        SELECT ae.id, ae.occurred_at, ae.decision, ae.reason_code,
               e.employee_no, e.display_name, z.name, d.name
        FROM access_events ae
        LEFT JOIN employees e ON e.id = ae.employee_id
        LEFT JOIN zones z ON z.id = ae.zone_id
        LEFT JOIN doors d ON d.id = ae.door_id
        WHERE 1=1
        "#,
    );
    if q.decision.is_some() {
        sql.push_str(" AND ae.decision = ?");
    }
    if q.since.is_some() {
        sql.push_str(" AND ae.occurred_at > ?");
    }
    let own_employee_id = if can_view_all_access_events(&session) {
        None
    } else {
        Some(employee_for_user(&state.db, session.user_id).await?.to_string())
    };
    if own_employee_id.is_some() {
        sql.push_str(" AND ae.employee_id = ?");
    } else if let Some(eno) = &q.employee_no {
        if eno.trim().is_empty() {
            return Err(ApiError::bad_request("employee_no must be non-empty"));
        }
        sql.push_str(" AND e.employee_no = ?");
    }
    sql.push_str(" ORDER BY ae.occurred_at DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )>(&sql);
    if let Some(decision) = &q.decision {
        query = query.bind(decision);
    }
    if let Some(since) = &q.since {
        if since.trim().is_empty() {
            return Err(ApiError::bad_request("since must be a non-empty RFC3339 timestamp"));
        }
        query = query.bind(since);
    }
    if let Some(ref eid) = own_employee_id {
        query = query.bind(eid);
    } else if let Some(eno) = &q.employee_no {
        query = query.bind(eno.trim());
    }
    query = query.bind(limit);
    let rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, occurred_at, decision, reason_code, employee_no, employee_name, zone_name, door_name)| {
                    AccessEventDto {
                        id,
                        occurred_at,
                        decision,
                        reason_code,
                        employee_no,
                        employee_name,
                        zone_name,
                        door_name,
                    }
                },
            )
            .collect(),
    ))
}

async fn simulate_scan(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<SimulateScanRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::HardwareDevice, Action::Read)?;

    // Process once here; do not also inject via hardware_sim (background worker would
    // duplicate the scan and can leave a deny/antipassback as the latest zone event).
    process_credential(&state, &body.credential_uid, &body.reader_id).await
}

#[derive(Serialize)]
struct HardwarePresentResponse {
    queued: bool,
    reader_id: String,
    credential_uid: String,
}

/// Queue a presentation on the hardware channel (same path as a physical reader).
/// Does not return grant/deny — poll `/access/events` or use `/simulate-scan` for immediate result.
async fn hardware_present(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<SimulateScanRequest>,
) -> Result<Json<HardwarePresentResponse>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::HardwareDevice, Action::Read)?;

    let event = HardwareEvent::CredentialPresented(RawCredentialPresentation {
        reader_id: body.reader_id.clone(),
        credential_uid: body.credential_uid.clone(),
        occurred_at: Utc::now(),
    });
    state
        .hardware_inject
        .send(event)
        .map_err(|_| ApiError::internal("Hardware-Kanal nicht verfügbar"))?;

    Ok(Json(HardwarePresentResponse {
        queued: true,
        reader_id: body.reader_id,
        credential_uid: body.credential_uid,
    }))
}

pub async fn process_credential(
    state: &AppState,
    credential_uid: &str,
    reader_id: &str,
) -> Result<Json<serde_json::Value>, ApiError> {
    let badge: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT id, employee_id FROM badges WHERE credential_uid = ? AND status = 'active'",
    )
    .bind(credential_uid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let door: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, zone_id, reader_in_id, reader_out_id FROM doors
        WHERE reader_in_id = ? OR reader_out_id = ? LIMIT 1
        "#,
    )
    .bind(reader_id)
    .bind(reader_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let (badge_id, employee_id) = badge.clone().unwrap_or((String::new(), None));

    let (decision, reason_code) = if badge.is_none() {
        ("deny", "unknown_badge")
    } else if door.is_none() {
        ("deny", "unknown_door")
    } else if employee_id.is_none() {
        ("deny", "unassigned_badge")
    } else {
        let (door_id, zone_id, reader_in, reader_out) = door.as_ref().unwrap();
        let emp = employee_id.as_deref().unwrap_or("");
        let eval = evaluate_access(
            &state.db,
            emp,
            zone_id.as_deref(),
            door_id,
            reader_id,
            reader_in.as_deref(),
            reader_out.as_deref(),
        )
        .await?;
        (eval.decision, eval.reason_code)
    };

    let event_id = Uuid::new_v4();
    let occurred_at = Utc::now().to_rfc3339();
    let (door_id, zone_id, _, _) = door.unwrap_or((String::new(), None, None, None));
    let badge_bind = if badge_id.is_empty() {
        None
    } else {
        Some(badge_id.as_str())
    };
    let door_bind = if door_id.is_empty() {
        None
    } else {
        Some(door_id.as_str())
    };

    sqlx::query(
        r#"
        INSERT INTO access_events (
            id, badge_id, employee_id, door_id, zone_id, decision, reason_code, occurred_at, raw_payload_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(event_id.to_string())
    .bind(badge_bind)
    .bind(employee_id.as_deref())
    .bind(door_bind)
    .bind(zone_id.as_deref())
    .bind(decision)
    .bind(reason_code)
    .bind(&occurred_at)
    .bind(serde_json::json!({ "reader_id": reader_id, "credential_uid": credential_uid }).to_string())
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let domain = DomainEvent {
        id: Uuid::new_v4(),
        topic: topics::ACCESS_DECISION.to_string(),
        schema_version: 1,
        occurred_at: Utc::now(),
        producer: "shard.access".into(),
        correlation_id: Some(event_id),
        actor: None,
        payload: serde_json::json!({
            "decision": decision,
            "reason_code": reason_code,
            "door_id": door_id,
            "credential_uid": credential_uid
        }),
    };

    sqlx::query(
        r#"
        INSERT INTO domain_events (id, topic, schema_version, occurred_at, producer, correlation_id, payload_json)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(domain.id.to_string())
    .bind(&domain.topic)
    .bind(1i64)
    .bind(domain.occurred_at.to_rfc3339())
    .bind(&domain.producer)
    .bind(domain.correlation_id.map(|u| u.to_string()))
    .bind(domain.payload.to_string())
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "event_id": event_id,
        "decision": decision,
        "reason_code": reason_code
    })))
}

/// Apply door status from hardware (TCP ingest / OEM bridge). Same allowed values as REST.
pub async fn process_door_state(
    state: &AppState,
    door_id: &str,
    status: &str,
) -> Result<(), ApiError> {
    if !matches!(
        status,
        "closed" | "open" | "forced_open" | "alarm"
    ) {
        return Err(ApiError::bad_request(
            "status: closed, open, forced_open, alarm",
        ));
    }

    let updated = sqlx::query("UPDATE doors SET status = ? WHERE id = ?")
        .bind(status)
        .bind(door_id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::not_found("Tür nicht gefunden"));
    }

    write_audit(
        &state.db,
        "hardware",
        None,
        "update",
        "door",
        Uuid::parse_str(door_id).ok(),
        Some("hardware ingest"),
        None,
        Some(serde_json::json!({ "status": status })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(())
}

#[derive(Deserialize)]
struct CreateZoneBody {
    name: String,
    #[serde(default)]
    site_id: Option<String>,
    #[serde(default = "default_risk")]
    risk_level: String,
}

fn default_risk() -> String {
    "normal".into()
}

#[derive(Deserialize)]
struct CreateDoorBody {
    name: String,
    zone_id: String,
    #[serde(default)]
    site_id: Option<String>,
    #[serde(default = "default_reader")]
    reader_id: String,
}

#[derive(Serialize)]
struct BadgeDto {
    id: String,
    employee_id: Option<String>,
    employee_no: Option<String>,
    employee_name: Option<String>,
    credential_uid: String,
    credential_type: String,
    status: String,
}

#[derive(Deserialize)]
struct CreateBadgeBody {
    employee_id: String,
    credential_uid: String,
    #[serde(default = "default_cred_type")]
    credential_type: String,
}

fn default_cred_type() -> String {
    "card".into()
}

async fn create_zone(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateZoneBody>,
) -> Result<Json<ZoneDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Zone, Action::Create)?;

    let site_id = match body.site_id {
        Some(s) => s,
        None => sqlx::query_scalar("SELECT id FROM sites LIMIT 1")
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .ok_or_else(|| ApiError::bad_request("Kein Standort konfiguriert"))?,
    };

    let id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO zones (id, site_id, name, risk_level, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(&site_id)
    .bind(&body.name)
    .bind(&body.risk_level)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(ZoneDto {
        id: id.to_string(),
        name: body.name,
        site_id,
        risk_level: body.risk_level,
    }))
}

async fn create_door(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateDoorBody>,
) -> Result<Json<DoorDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Door, Action::Create)?;

    let site_id = match body.site_id {
        Some(s) => s,
        None => sqlx::query_scalar("SELECT site_id FROM zones WHERE id = ?")
            .bind(&body.zone_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?
            .ok_or_else(|| ApiError::bad_request("Zone nicht gefunden"))?,
    };

    let id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO doors (id, site_id, zone_id, name, direction, status, reader_in_id, created_at)
        VALUES (?, ?, ?, ?, 'in', 'closed', ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(&site_id)
    .bind(&body.zone_id)
    .bind(&body.name)
    .bind(&body.reader_id)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(DoorDto {
        id: id.to_string(),
        name: body.name,
        zone_id: Some(body.zone_id),
        status: "closed".into(),
        reader_in_id: Some(body.reader_id),
        reader_out_id: None,
    }))
}

async fn list_badges(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<BadgeDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Badge, Action::Read)?;

    let rows: Vec<(
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
    )> = if can_view_all_access_events(&session) {
        sqlx::query_as(
            r#"
            SELECT b.id, b.employee_id, e.employee_no, e.display_name,
                   b.credential_uid, b.credential_type, b.status
            FROM badges b
            LEFT JOIN employees e ON e.id = b.employee_id
            ORDER BY b.issued_at DESC
            LIMIT 200
            "#,
        )
        .fetch_all(&state.db)
        .await
    } else {
        let emp = employee_for_user(&state.db, session.user_id).await?;
        sqlx::query_as(
            r#"
            SELECT b.id, b.employee_id, e.employee_no, e.display_name,
                   b.credential_uid, b.credential_type, b.status
            FROM badges b
            LEFT JOIN employees e ON e.id = b.employee_id
            WHERE b.employee_id = ?
            ORDER BY b.issued_at DESC
            "#,
        )
        .bind(emp.to_string())
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, employee_id, employee_no, employee_name, credential_uid, credential_type, status)| {
                    BadgeDto {
                        id,
                        employee_id,
                        employee_no,
                        employee_name,
                        credential_uid,
                        credential_type,
                        status,
                    }
                },
            )
            .collect(),
    ))
}

async fn create_badge(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateBadgeBody>,
) -> Result<Json<BadgeDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Badge, Action::Create)?;

    let id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO badges (id, employee_id, credential_uid, credential_type, status, issued_at)
        VALUES (?, ?, ?, ?, 'active', ?)
        "#,
    )
    .bind(id.to_string())
    .bind(&body.employee_id)
    .bind(&body.credential_uid)
    .bind(&body.credential_type)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            ApiError::bad_request("Credential UID bereits vergeben")
        } else {
            ApiError::internal(e.to_string())
        }
    })?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "create",
        "badge",
        Some(id),
        None,
        None,
        Some(serde_json::json!({ "credential_uid": body.credential_uid })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    fetch_badge(&state.db, &id.to_string()).await
}

async fn revoke_badge(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<BadgeDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Badge, Action::Update)?;
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM badges WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if status.as_deref() != Some("active") {
        return Err(ApiError::bad_request("Badge ist nicht aktiv"));
    }
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE badges SET status = 'revoked', revoked_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "revoke",
        "badge",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    fetch_badge(&state.db, &id).await
}

async fn fetch_badge(pool: &sqlx::SqlitePool, id: &str) -> Result<Json<BadgeDto>, ApiError> {
    let row: Option<(
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
    )> = sqlx::query_as(
        r#"
        SELECT b.id, b.employee_id, e.employee_no, e.display_name,
               b.credential_uid, b.credential_type, b.status
        FROM badges b
        LEFT JOIN employees e ON e.id = b.employee_id
        WHERE b.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    let Some((id, employee_id, employee_no, employee_name, credential_uid, credential_type, status)) =
        row
    else {
        return Err(ApiError::not_found("Badge nicht gefunden"));
    };
    Ok(Json(BadgeDto {
        id,
        employee_id,
        employee_no,
        employee_name,
        credential_uid,
        credential_type,
        status,
    }))
}

#[derive(Serialize)]
struct AccessRuleDto {
    id: String,
    principal_type: String,
    principal_id: String,
    employee_name: Option<String>,
    zone_id: Option<String>,
    zone_name: Option<String>,
    door_id: Option<String>,
    mode: String,
    valid_from: String,
    valid_to: Option<String>,
    schedule_json: Option<String>,
}

#[derive(Deserialize)]
struct UpdateAccessRuleBody {
    #[serde(default)]
    schedule_json: Option<Option<String>>,
    #[serde(default)]
    valid_from: Option<Option<String>>,
    #[serde(default)]
    valid_to: Option<Option<String>>,
}

#[derive(Deserialize)]
struct CreateAccessRuleBody {
    employee_id: String,
    zone_id: String,
    #[serde(default)]
    door_id: Option<String>,
    #[serde(default)]
    valid_from: Option<String>,
    #[serde(default)]
    valid_to: Option<String>,
    #[serde(default)]
    schedule_json: Option<String>,
}

async fn list_access_rules(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<AccessRuleDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::AccessRule, Action::Read)?;

    let rows: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT r.id, r.principal_type, r.principal_id, e.display_name, r.zone_id, z.name,
               r.door_id, r.mode, r.valid_from, r.valid_to, r.schedule_json
        FROM access_rules r
        LEFT JOIN employees e ON e.id = r.principal_id AND r.principal_type = 'employee'
        LEFT JOIN zones z ON z.id = r.zone_id
        ORDER BY r.valid_from DESC
        LIMIT 200
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, pt, pid, en, zid, zn, did, mode, vf, vt, sched)| AccessRuleDto {
                    id,
                    principal_type: pt,
                    principal_id: pid,
                    employee_name: en,
                    zone_id: zid,
                    zone_name: zn,
                    door_id: did,
                    mode,
                    valid_from: vf,
                    valid_to: vt,
                    schedule_json: sched,
                },
            )
            .collect(),
    ))
}

async fn create_access_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateAccessRuleBody>,
) -> Result<Json<AccessRuleDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::AccessRule, Action::Create)?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let valid_from = body.valid_from.as_ref().unwrap_or(&now);
    sqlx::query(
        r#"
        INSERT INTO access_rules (
            id, principal_type, principal_id, zone_id, door_id, schedule_json,
            valid_from, valid_to, mode, created_at
        ) VALUES (?, 'employee', ?, ?, ?, ?, ?, ?, 'allow', ?)
        "#,
    )
    .bind(&id)
    .bind(&body.employee_id)
    .bind(&body.zone_id)
    .bind(&body.door_id)
    .bind(&body.schedule_json)
    .bind(valid_from)
    .bind(&body.valid_to)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "create",
        "access_rule",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        Some(serde_json::json!({
            "employee_id": body.employee_id,
            "zone_id": body.zone_id
        })),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let rows: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT r.id, r.principal_type, r.principal_id, e.display_name, r.zone_id, z.name,
               r.door_id, r.mode, r.valid_from, r.valid_to, r.schedule_json
        FROM access_rules r
        LEFT JOIN employees e ON e.id = r.principal_id
        LEFT JOIN zones z ON z.id = r.zone_id
        WHERE r.id = ?
        "#,
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some((id, pt, pid, en, zid, zn, did, mode, vf, vt, sched)) = rows.into_iter().next() else {
        return Err(ApiError::internal("Regel nicht geladen"));
    };
    Ok(Json(AccessRuleDto {
        id,
        principal_type: pt,
        principal_id: pid,
        employee_name: en,
        zone_id: zid,
        zone_name: zn,
        door_id: did,
        mode,
        valid_from: vf,
        valid_to: vt,
        schedule_json: sched,
    }))
}

async fn update_access_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateAccessRuleBody>,
) -> Result<Json<AccessRuleDto>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::AccessRule, Action::Update)?;

    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM access_rules WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if exists.is_none() {
        return Err(ApiError::not_found("Zutrittsregel nicht gefunden"));
    }

    if body.schedule_json.is_some() {
        sqlx::query("UPDATE access_rules SET schedule_json = ? WHERE id = ?")
            .bind(body.schedule_json.as_ref().and_then(|o| o.clone()))
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    if body.valid_from.is_some() {
        sqlx::query("UPDATE access_rules SET valid_from = ? WHERE id = ?")
            .bind(body.valid_from.as_ref().and_then(|o| o.clone()))
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    if body.valid_to.is_some() {
        sqlx::query("UPDATE access_rules SET valid_to = ? WHERE id = ?")
            .bind(body.valid_to.as_ref().and_then(|o| o.clone()))
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "update",
        "access_rule",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let rows: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT r.id, r.principal_type, r.principal_id, e.display_name, r.zone_id, z.name,
               r.door_id, r.mode, r.valid_from, r.valid_to, r.schedule_json
        FROM access_rules r
        LEFT JOIN employees e ON e.id = r.principal_id AND r.principal_type = 'employee'
        LEFT JOIN zones z ON z.id = r.zone_id
        WHERE r.id = ?
        "#,
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let Some((id, pt, pid, en, zid, zn, did, mode, vf, vt, sched)) = rows.into_iter().next() else {
        return Err(ApiError::internal("Regel nicht geladen"));
    };
    Ok(Json(AccessRuleDto {
        id,
        principal_type: pt,
        principal_id: pid,
        employee_name: en,
        zone_id: zid,
        zone_name: zn,
        door_id: did,
        mode,
        valid_from: vf,
        valid_to: vt,
        schedule_json: sched,
    }))
}

async fn delete_access_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::AccessRule, Action::Delete)?;

    let result = sqlx::query("DELETE FROM access_rules WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("Zutrittsregel nicht gefunden"));
    }

    write_audit(
        &state.db,
        "user",
        Some(session.user_id),
        "delete",
        "access_rule",
        Uuid::parse_str(&id).ok(),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Serialize)]
struct ZoneOccupancyDto {
    zone_id: String,
    zone_name: String,
    inside_count: usize,
    occupants: Vec<OccupantDto>,
}

#[derive(Serialize)]
struct OccupantDto {
    employee_id: String,
    employee_no: String,
    display_name: String,
}

async fn zone_occupancy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<ZoneOccupancyDto>>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    require_permission(&session, Resource::Zone, Action::Read)?;

    let zones: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM zones ORDER BY name")
            .fetch_all(&state.db)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

    let employees: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, employee_no, display_name FROM employees WHERE active_to IS NULL",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut result = Vec::new();
    for (zone_id, zone_name) in zones {
        let mut occupants = Vec::new();
        for (emp_id, emp_no, emp_name) in &employees {
            if employee_inside_zone(&state.db, emp_id, &zone_id).await? {
                occupants.push(OccupantDto {
                    employee_id: emp_id.clone(),
                    employee_no: emp_no.clone(),
                    display_name: emp_name.clone(),
                });
            }
        }
        result.push(ZoneOccupancyDto {
            zone_id,
            zone_name,
            inside_count: occupants.len(),
            occupants,
        });
    }
    Ok(Json(result))
}

#[derive(Serialize)]
struct ReaderOptionDto {
    id: String,
    label: String,
}

#[derive(Serialize)]
struct MyAccessSummary {
    badges: Vec<BadgeDto>,
    recent_events: Vec<serde_json::Value>,
    /// Configured door readers for simulate-scan (employee app).
    readers: Vec<ReaderOptionDto>,
}

async fn my_access_summary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<MyAccessSummary>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    let emp_id = employee_for_user(&state.db, session.user_id).await?;

    let badge_rows: Vec<(
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
    )> = sqlx::query_as(
        r#"
        SELECT b.id, b.employee_id, e.employee_no, e.display_name,
               b.credential_uid, b.credential_type, b.status
        FROM badges b
        LEFT JOIN employees e ON e.id = b.employee_id
        WHERE b.employee_id = ?
        "#,
    )
    .bind(emp_id.to_string())
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let badges: Vec<BadgeDto> = badge_rows
        .into_iter()
        .map(
            |(id, employee_id, employee_no, employee_name, credential_uid, credential_type, status)| {
                BadgeDto {
                    id,
                    employee_id,
                    employee_no,
                    employee_name,
                    credential_uid,
                    credential_type,
                    status,
                }
            },
        )
        .collect();

    let event_rows: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT ae.id, ae.decision, ae.reason_code, z.name, ae.occurred_at
        FROM access_events ae
        LEFT JOIN zones z ON z.id = ae.zone_id
        WHERE ae.employee_id = ?
        ORDER BY ae.occurred_at DESC
        LIMIT 20
        "#,
    )
    .bind(emp_id.to_string())
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let recent_events: Vec<serde_json::Value> = event_rows
        .into_iter()
        .map(|(id, decision, reason_code, zone_name, occurred_at)| {
            serde_json::json!({
                "id": id,
                "decision": decision,
                "reason_code": reason_code,
                "zone_name": zone_name,
                "occurred_at": occurred_at
            })
        })
        .collect();

    let door_rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT name, reader_in_id, reader_out_id FROM doors ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut readers = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (name, reader_in_id, reader_out_id) in door_rows {
        if let Some(id) = reader_in_id.filter(|s| !s.is_empty()) {
            if seen.insert(id.clone()) {
                readers.push(ReaderOptionDto {
                    id: id.clone(),
                    label: format!("{name} — Eingang"),
                });
            }
        }
        if let Some(id) = reader_out_id.filter(|s| !s.is_empty()) {
            if seen.insert(id.clone()) {
                readers.push(ReaderOptionDto {
                    id: id.clone(),
                    label: format!("{name} — Ausgang"),
                });
            }
        }
    }

    Ok(Json(MyAccessSummary {
        badges,
        recent_events,
        readers,
    }))
}

async fn my_simulate_scan(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<SimulateScanRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    let emp_id = employee_for_user(&state.db, session.user_id).await?.to_string();

    let owned: Option<String> = sqlx::query_scalar(
        "SELECT credential_uid FROM badges WHERE employee_id = ? AND credential_uid = ? AND status = 'active'",
    )
    .bind(&emp_id)
    .bind(&body.credential_uid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    if owned.is_none() {
        return Err(ApiError::forbidden(
            "Nur das eigene aktive Badge darf simuliert werden",
        ));
    }

    process_credential(&state, &body.credential_uid, &body.reader_id).await
}
