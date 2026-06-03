use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use chrono_tz::Europe::Berlin;
use std::sync::Arc;
use timeshards_core::permissions::{Action, Resource};
use timeshards_core::ApiError;

use crate::auth::{auth_from_headers, can_manage_others, can_view_all_access_events};
use crate::routes::time::employee_for_user;
use crate::state::AppState;
use timeshards_db::WeekEvaluationMeta;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/reports/timesheets/export", get(export_timesheets))
        .route("/api/v1/reports/payroll/export", get(export_payroll))
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

type TimesheetExportRow = (
    String,
    String,
    String,
    String,
    String,
    i64,
    i64,
    i64,
    i64,
    String,
    Option<String>,
);

fn month_bounds_rfc3339(year: i32, month: u32) -> Result<(String, String), ApiError> {
    if !(1..=12).contains(&month) {
        return Err(ApiError::bad_request("Monat muss 1–12 sein"));
    }
    let start_date = chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| ApiError::bad_request("Ungültiges Jahr/Monat"))?;
    let end_date = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .ok_or_else(|| ApiError::bad_request("Ungültiges Jahr/Monat"))?;
    let start = start_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Berlin)
        .single()
        .ok_or_else(|| ApiError::internal("Berlin timezone"))?
        .with_timezone(&chrono::Utc);
    let end = end_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Berlin)
        .single()
        .ok_or_else(|| ApiError::internal("Berlin timezone"))?
        .with_timezone(&chrono::Utc);
    Ok((start.to_rfc3339(), end.to_rfc3339()))
}

fn credited_minutes_from_eval(json: Option<&str>) -> i64 {
    let Some(j) = json else {
        return 0;
    };
    serde_json::from_str::<WeekEvaluationMeta>(j)
        .ok()
        .map(|m| m.settlement.credited_minutes)
        .unwrap_or(0)
}

#[derive(serde::Deserialize)]
struct PayrollExportQuery {
    year: i32,
    month: u32,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    employee_id: Option<String>,
    /// `employee` = one summary row per MA; default = one row per approved KW
    #[serde(default)]
    aggregate: Option<String>,
}

async fn export_payroll(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<PayrollExportQuery>,
) -> Result<Response, ApiError> {
    let session = auth_from_headers(&state.db, &headers).await?;
    if !session.permissions.allows(Resource::Report, Action::Export)
        && !session.permissions.allows(Resource::Timesheet, Action::Approve)
    {
        return Err(ApiError::forbidden("Keine Export-Berechtigung"));
    }

    let (from, to) = month_bounds_rfc3339(q.year, q.month)?;
    let aggregate = q.aggregate.as_deref() == Some("employee");

    let own_employee_id = if can_manage_others(&session) {
        None
    } else {
        Some(employee_for_user(&state.db, session.user_id).await?.to_string())
    };

    let filter_emp = q.employee_id.as_ref().or(own_employee_id.as_ref());

    let mut sql = String::from(
        r#"
        SELECT e.employee_no, e.display_name, e.id, t.period_start,
               t.worked_minutes, t.expected_minutes, t.balance_minutes,
               t.overtime_minutes, t.evaluation_json
        FROM timesheets t
        JOIN employees e ON e.id = t.employee_id
        WHERE t.status = 'approved' AND t.period_start >= ? AND t.period_start < ?
        "#,
    );
    if filter_emp.is_some() {
        sql.push_str(" AND t.employee_id = ?");
    }
    sql.push_str(" ORDER BY e.employee_no, t.period_start");

    let mut query = sqlx::query_as::<_, (
        String,
        String,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        Option<String>,
    )>(&sql)
    .bind(&from)
    .bind(&to);
    if let Some(eid) = filter_emp {
        query = query.bind(eid);
    }
    let week_rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let account_rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT employee_id, account_kind, balance_minutes FROM time_accounts",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut flex_by_emp: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    let mut ot_by_emp: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for (emp_id, kind, bal) in account_rows {
        if kind == "flex" {
            flex_by_emp.insert(emp_id.clone(), bal);
        } else if kind == "overtime" {
            ot_by_emp.insert(emp_id, bal);
        }
    }

    // UTF-8 BOM helps Excel (DE) open semicolon CSV with correct encoding.
    let mut out = String::from(
        "\u{feff}personal_nr;name;jahr;monat;kw_beginn;ist_min;ist_h;soll_min;soll_h;saldo_min;saldo_h;gutschrift_min;ueberstunden_min;gleitzeit_konto_min;ueberstunden_konto_min\n",
    );

    if aggregate {
        struct Agg {
            no: String,
            name: String,
            emp_id: String,
            worked: i64,
            expected: i64,
            balance: i64,
            credited: i64,
            overtime: i64,
        }
        let mut by_emp: std::collections::HashMap<String, Agg> =
            std::collections::HashMap::new();
        for (no, name, emp_id, _ps, worked, expected, balance, overtime, eval_json) in week_rows
        {
            let credited = credited_minutes_from_eval(eval_json.as_deref());
            by_emp
                .entry(emp_id.clone())
                .and_modify(|a| {
                    a.worked += worked;
                    a.expected += expected;
                    a.balance += balance;
                    a.credited += credited;
                    a.overtime += overtime;
                })
                .or_insert(Agg {
                    no: no.clone(),
                    name: name.clone(),
                    emp_id: emp_id.clone(),
                    worked,
                    expected,
                    balance,
                    credited,
                    overtime,
                });
        }
        for a in by_emp.values() {
            push_payroll_row(
                &mut out,
                &a.no,
                &a.name,
                q.year,
                q.month,
                "",
                a.worked,
                a.expected,
                a.balance,
                a.credited,
                a.overtime,
                flex_by_emp.get(&a.emp_id).copied().unwrap_or(0),
                ot_by_emp.get(&a.emp_id).copied().unwrap_or(0),
            );
        }
    } else {
        for (no, name, emp_id, ps, worked, expected, balance, overtime, eval_json) in week_rows {
            let credited = credited_minutes_from_eval(eval_json.as_deref());
            push_payroll_row(
                &mut out,
                &no,
                &name,
                q.year,
                q.month,
                &ps,
                worked,
                expected,
                balance,
                credited,
                overtime,
                flex_by_emp.get(&emp_id).copied().unwrap_or(0),
                ot_by_emp.get(&emp_id).copied().unwrap_or(0),
            );
        }
    }

    if out.lines().count() <= 1 {
        out.push_str("(keine freigegebenen Wochen im Monat)\n");
    }

    if q.format != "csv" {
        return Err(ApiError::bad_request("Nur format=csv unterstützt"));
    }
    let filename = format!("lohn_export_{}_{:02}.csv", q.year, q.month);
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    let mut resp = (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        out,
    )
        .into_response();
    resp.headers_mut()
        .insert(header::CONTENT_DISPOSITION, disposition);
    Ok(resp)
}

#[allow(clippy::too_many_arguments)]
fn push_payroll_row(
    out: &mut String,
    no: &str,
    name: &str,
    year: i32,
    month: u32,
    period_start: &str,
    worked: i64,
    expected: i64,
    balance: i64,
    credited: i64,
    overtime: i64,
    flex_account: i64,
    ot_account: i64,
) {
    let worked_h = worked as f64 / 60.0;
    let expected_h = expected as f64 / 60.0;
    let balance_h = balance as f64 / 60.0;
    out.push_str(&format!(
        "{no};{name};{year};{month};{period_start};{worked};{worked_h:.2};{expected};{expected_h:.2};{balance};{balance_h:.2};{credited};{overtime};{flex_account};{ot_account}\n",
    ));
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
               t.worked_minutes, t.expected_minutes, t.balance_minutes, t.overtime_minutes,
               t.status, t.evaluation_json
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
    let rows: Vec<TimesheetExportRow> = query
    .fetch_all(&state.db)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    match q.format.as_str() {
        "html" => Ok(html_response(&rows, &status)),
        "csv" | _ => Ok(csv_response(&rows, &status)),
    }
}

fn csv_response(rows: &[TimesheetExportRow], status: &str) -> Response {
    let mut out = String::from(
        "employee_no;display_name;period_start;period_end;worked_min;worked_h;expected_min;expected_h;balance_min;balance_h;overtime_min;overtime_h;status\n",
    );
    for (_, no, name, ps, pe, wm, em, bm, om, st, _) in rows {
        out.push_str(&format!(
            "{no};{name};{ps};{pe};{wm};{:.2};{em};{:.2};{bm};{:.2};{om};{:.2};{st}\n",
            *wm as f64 / 60.0,
            *em as f64 / 60.0,
            *bm as f64 / 60.0,
            *om as f64 / 60.0,
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

fn format_minutes_hm(mins: i64) -> String {
    let h = mins / 60;
    let m = mins.abs() % 60;
    format!("{h}:{m:02}")
}

fn evaluation_days_html(evaluation_json: &Option<String>) -> String {
    let Some(raw) = evaluation_json else {
        return String::new();
    };
    let Ok(meta) = serde_json::from_str::<WeekEvaluationMeta>(raw) else {
        return String::new();
    };
    if meta.days.is_empty() {
        return String::new();
    }
    let mut days_rows = String::new();
    for d in &meta.days {
        let absence = d
            .absence_label
            .as_deref()
            .map(|l| format!(" · {l}"))
            .unwrap_or_default();
        days_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}{absence}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            d.date,
            d.model_name,
            format_minutes_hm(d.expected_minutes),
            format_minutes_hm(d.worked_minutes),
            format_minutes_hm(d.balance_minutes),
        ));
    }
    let cal = html_escape(&meta.work_calendar_name);
    format!(
        r#"<h3>Tagesdetails — {cal}</h3>
<table class="days"><thead><tr><th>Tag</th><th>Modell</th><th>Soll</th><th>Ist</th><th>Saldo</th></tr></thead>
<tbody>{days_rows}</tbody></table>"#
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn html_response(rows: &[TimesheetExportRow], status: &str) -> Response {
    let mut sections = String::new();
    for (_, no, name, ps, pe, wm, em, bm, om, st, eval_json) in rows {
        let no_e = html_escape(no);
        let name_e = html_escape(name);
        let days = evaluation_days_html(eval_json);
        sections.push_str(&format!(
            r#"<section class="sheet">
<h2>{no_e} — {name_e}</h2>
<p class="meta">{ps} – {pe} · Status: {st}</p>
<table class="summary"><tbody>
<tr><th>Ist</th><td>{:.2} h ({})</td></tr>
<tr><th>Soll</th><td>{:.2} h ({})</td></tr>
<tr><th>Saldo</th><td>{:.2} h ({})</td></tr>
<tr><th>Überstunden</th><td>{:.2} h</td></tr>
</tbody></table>
{days}
</section>
"#,
            *wm as f64 / 60.0,
            format_minutes_hm(*wm),
            *em as f64 / 60.0,
            format_minutes_hm(*em),
            *bm as f64 / 60.0,
            format_minutes_hm(*bm),
            *om as f64 / 60.0,
        ));
    }
    if rows.is_empty() {
        sections.push_str("<p>Keine Einträge</p>");
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="de"><head><meta charset="utf-8"><title>Stundenzettel</title>
<style>
body{{font-family:Segoe UI,sans-serif;margin:2rem;color:#111}}
table{{border-collapse:collapse;width:100%;margin:0.5rem 0}}
th,td{{border:1px solid #ccc;padding:6px 8px;text-align:left}}
th{{background:#eee}}
section.sheet{{page-break-after:always;margin-bottom:2rem}}
section.sheet:last-child{{page-break-after:auto}}
.meta{{color:#555;font-size:0.95rem}}
.days{{font-size:0.9rem}}
</style></head>
<body><h1>Stundenzettel ({status})</h1>
<p>AI TimeShards — Drucken → Als PDF speichern. Tagesdetails erscheinen nach Neuberechnung mit Arbeitskalender.</p>
{sections}</body></html>"#
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
