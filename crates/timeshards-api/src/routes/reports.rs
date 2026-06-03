use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use timeshards_core::permissions::{Action, Resource};
use timeshards_core::ApiError;

use crate::auth::{auth_from_headers, can_manage_others, can_view_all_access_events};
use crate::routes::time::employee_for_user;
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/reports/timesheets/export", get(export_timesheets))
        .route("/api/v1/reports/access/export", get(export_access_events))
}

#[derive(serde::Deserialize)]
struct ExportQuery {
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    status: Option<String>,
    /// ISO period start (Monday 00:00) — only timesheets for that week
    #[serde(default)]
    period_start: Option<String>,
}

fn default_format() -> String {
    "csv".into()
}

async fn export_timesheets(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    if !session.permissions.allows(Resource::Report, Action::Export)
        && !session.permissions.allows(Resource::Timesheet, Action::Read)
    {
        return Err(ApiError::forbidden("Keine Export-Berechtigung"));
    }

    let status = q.status.unwrap_or_else(|| "approved".into());

    let mut sql = String::from(
        r#"
        SELECT t.id, e.employee_no, e.display_name, t.period_start, t.period_end,
               t.worked_minutes, t.overtime_minutes, t.status
        FROM timesheets t
        JOIN employees e ON e.id = t.employee_id
        WHERE t.status = ?
        "#,
    );
    if q.period_start.is_some() {
        sql.push_str(" AND t.period_start = ?");
    }
    let own_employee_id = if can_manage_others(&session) {
        None
    } else {
        Some(employee_for_user(&state.db, session.user_id).await?.to_string())
    };
    if own_employee_id.is_some() {
        sql.push_str(" AND t.employee_id = ?");
    }
    sql.push_str(" ORDER BY e.employee_no, t.period_start");

    let mut query = sqlx::query_as(&sql).bind(&status);
    if let Some(ref ps) = q.period_start {
        query = query.bind(ps);
    }
    if let Some(ref eid) = own_employee_id {
        query = query.bind(eid);
    }
    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        String,
    )> = query
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    match q.format.as_str() {
        "html" => Ok(html_response(&rows, &status)),
        "csv" | _ => Ok(csv_response(&rows, &status)),
    }
}

fn csv_response(
    rows: &[(
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        String,
    )],
    status: &str,
) -> Response {
    let mut out = String::from(
        "employee_no;display_name;period_start;period_end;worked_hours;overtime_hours;status\n",
    );
    for (_, no, name, ps, pe, wm, om, st) in rows {
        let wh = *wm as f64 / 60.0;
        let oh = *om as f64 / 60.0;
        out.push_str(&format!(
            "{no};{name};{ps};{pe};{wh:.2};{oh:.2};{st}\n"
        ));
    }
    if rows.is_empty() {
        out.push_str("(keine Einträge)\n");
    }

    let disposition = HeaderValue::from_str(&format!(
        "attachment; filename=\"stundenzettel_{status}.csv\""
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("attachment"));

    let mut resp = (StatusCode::OK, [(header::CONTENT_TYPE, "text/csv; charset=utf-8")], out)
        .into_response();
    resp.headers_mut()
        .insert(header::CONTENT_DISPOSITION, disposition);
    resp
}

fn html_response(
    rows: &[(
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        String,
    )],
    status: &str,
) -> Response {
    let mut rows_html = String::new();
    for (_, no, name, ps, pe, wm, om, st) in rows {
        rows_html.push_str(&format!(
            "<tr><td>{no}</td><td>{name}</td><td>{ps}</td><td>{pe}</td><td>{:.2}</td><td>{:.2}</td><td>{st}</td></tr>\n",
            *wm as f64 / 60.0,
            *om as f64 / 60.0,
        ));
    }
    if rows.is_empty() {
        rows_html.push_str("<tr><td colspan=\"7\">Keine Einträge</td></tr>");
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="de"><head><meta charset="utf-8"><title>Stundenzettel</title>
<style>body{{font-family:Segoe UI,sans-serif;margin:2rem}}table{{border-collapse:collapse;width:100%}}
th,td{{border:1px solid #ccc;padding:8px;text-align:left}}th{{background:#eee}}</style></head>
<body><h1>Stundenzettel ({status})</h1><p>AI TimeShards — zum PDF: Drucken → Als PDF speichern</p>
<table><thead><tr><th>PN</th><th>Name</th><th>Von</th><th>Bis</th><th>Stunden</th><th>Überstunden</th><th>Status</th></tr></thead>
<tbody>{rows_html}</tbody></table></body></html>"#
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct AccessExportQuery {
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    500
}

async fn export_access_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<AccessExportQuery>,
) -> Result<Response, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    if !session.permissions.allows(Resource::Report, Action::Export)
        && !session.permissions.allows(Resource::AccessEvent, Action::Read)
    {
        return Err(ApiError::forbidden("Keine Export-Berechtigung"));
    }

    let limit = q.limit.min(2000);
    let mut sql = String::from(
        r#"
        SELECT ae.occurred_at, e.employee_no, e.display_name, z.name, d.name,
               ae.decision, ae.reason_code, ae.raw_payload_json
        FROM access_events ae
        LEFT JOIN employees e ON e.id = ae.employee_id
        LEFT JOIN zones z ON z.id = ae.zone_id
        LEFT JOIN doors d ON d.id = ae.door_id
        WHERE 1=1
        "#,
    );
    if q.from.is_some() {
        sql.push_str(" AND ae.occurred_at >= ?");
    }
    if q.to.is_some() {
        sql.push_str(" AND ae.occurred_at < ?");
    }
    let own_employee_id = if can_view_all_access_events(&session) {
        None
    } else {
        Some(employee_for_user(&state.db, session.user_id).await?.to_string())
    };
    if own_employee_id.is_some() {
        sql.push_str(" AND ae.employee_id = ?");
    }
    sql.push_str(" ORDER BY ae.occurred_at DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    )>(&sql);
    if let Some(from) = &q.from {
        query = query.bind(from);
    }
    if let Some(to) = &q.to {
        query = query.bind(to);
    }
    if let Some(ref eid) = own_employee_id {
        query = query.bind(eid);
    }
    query = query.bind(limit);
    let rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    match q.format.as_str() {
        "html" => Ok(access_html_response(&rows)),
        "csv" | _ => Ok(access_csv_response(&rows)),
    }
}

type AccessRow = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
);

fn reader_from_raw(raw: &Option<String>) -> String {
    raw.as_ref()
        .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
        .and_then(|v| v.get("reader_id").and_then(|r| r.as_str().map(str::to_string)))
        .unwrap_or_default()
}

fn access_csv_response(rows: &[AccessRow]) -> Response {
    let mut out = String::from(
        "occurred_at;employee_no;employee_name;zone;door;decision;reason_code;reader_id\n",
    );
    for (at, eno, ename, zone, door, decision, reason, raw) in rows {
        let reader = reader_from_raw(raw);
        out.push_str(&format!(
            "{at};{};{};{};{};{decision};{};{reader}\n",
            eno.as_deref().unwrap_or(""),
            ename.as_deref().unwrap_or(""),
            zone.as_deref().unwrap_or(""),
            door.as_deref().unwrap_or(""),
            reason.as_deref().unwrap_or(""),
        ));
    }
    if rows.is_empty() {
        out.push_str("(keine Einträge)\n");
    }
    let disposition = HeaderValue::from_str("attachment; filename=\"zutritt_protokoll.csv\"")
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    let mut resp = (StatusCode::OK, [(header::CONTENT_TYPE, "text/csv; charset=utf-8")], out)
        .into_response();
    resp.headers_mut()
        .insert(header::CONTENT_DISPOSITION, disposition);
    resp
}

fn access_html_response(rows: &[AccessRow]) -> Response {
    let mut rows_html = String::new();
    for (at, eno, ename, zone, door, decision, reason, raw) in rows {
        let reader = reader_from_raw(raw);
        rows_html.push_str(&format!(
            "<tr><td>{at}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{decision}</td><td>{}</td><td>{reader}</td></tr>\n",
            eno.as_deref().unwrap_or("—"),
            ename.as_deref().unwrap_or("—"),
            zone.as_deref().unwrap_or("—"),
            door.as_deref().unwrap_or("—"),
            reason.as_deref().unwrap_or("—"),
        ));
    }
    if rows.is_empty() {
        rows_html.push_str("<tr><td colspan=\"8\">Keine Einträge</td></tr>");
    }
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="de"><head><meta charset="utf-8"><title>Zutrittprotokoll</title>
<style>body{{font-family:Segoe UI,sans-serif;margin:2rem}}table{{border-collapse:collapse;width:100%}}
th,td{{border:1px solid #ccc;padding:8px;text-align:left}}th{{background:#eee}}
.deny{{color:#b00020}}</style></head>
<body><h1>Zutrittprotokoll</h1><p>AI TimeShards — Drucken → Als PDF speichern</p>
<table><thead><tr><th>Zeit</th><th>PN</th><th>Name</th><th>Zone</th><th>Tür</th><th>Entscheidung</th><th>Grund</th><th>Leser</th></tr></thead>
<tbody>{rows_html}</tbody></table></body></html>"#
    );
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}
