use crate::hash_password;
use crate::timesheet_compute::{upsert_draft_timesheet_for_week, week_bounds_utc};
use crate::work_calendar_seed::{assign_all_active_employees, ensure_work_calendar_foundation};
use chrono::{Duration, NaiveTime, Timelike, Utc};
use chrono_tz::Tz;
use sqlx::SqlitePool;
use timeshards_core::permissions::RoleTemplate;
use tracing::info;
use uuid::Uuid;

const DEFAULT_ADMIN_USER: &str = "admin";
const DEFAULT_ADMIN_PASS: &str = "admin";

/// Initial admin password on empty DB (`TIMESHARDS_ADMIN_PASSWORD` or `admin`).
pub fn initial_admin_password() -> String {
    std::env::var("TIMESHARDS_ADMIN_PASSWORD")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_ADMIN_PASS.into())
}

pub async fn seed_if_empty(pool: &SqlitePool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }

    info!("seeding default site, roles, and admin user");

    // Work calendar models are also ensured on every API start; seed early on empty DB.
    ensure_work_calendar_foundation(pool).await?;

    let now = Utc::now().to_rfc3339();
    let site_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sites (id, name, timezone, created_at) VALUES (?, ?, ?, ?)")
        .bind(&site_id)
        .bind("Hauptstandort")
        .bind("Europe/Berlin")
        .bind(&now)
        .execute(pool)
        .await?;

    for template in [
        RoleTemplate::SystemAdmin,
        RoleTemplate::HrAdmin,
        RoleTemplate::SecurityOperator,
        RoleTemplate::Manager,
        RoleTemplate::Employee,
    ] {
        let role_id = Uuid::new_v4().to_string();
        let perms: Vec<String> = template.permissions().keys().cloned().collect();
        let perms_json = serde_json::to_string(&perms)?;
        let name = template_key(template);
        sqlx::query(
            "INSERT INTO roles (id, name, template_key, permissions_json, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&role_id)
        .bind(name)
        .bind(name)
        .bind(&perms_json)
        .bind(&now)
        .execute(pool)
        .await?;

        if template == RoleTemplate::SystemAdmin {
            let user_id = Uuid::new_v4().to_string();
            let admin_pass = initial_admin_password();
            let hash = hash_password(&admin_pass)?;
            sqlx::query(
                r#"
                INSERT INTO users (id, username, display_name, email, password_hash, locale, status, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&user_id)
            .bind(DEFAULT_ADMIN_USER)
            .bind("Administrator")
            .bind("admin@local")
            .bind(&hash)
            .bind("de")
            .bind("active")
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;

            sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
                .bind(&user_id)
                .bind(&role_id)
                .execute(pool)
                .await?;

            let emp_id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO employees (id, user_id, employee_no, display_name, active_from, created_at)
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&emp_id)
            .bind(&user_id)
            .bind("0001")
            .bind("Administrator")
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;

            let badge_id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO badges (id, employee_id, credential_uid, credential_type, status, issued_at)
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&badge_id)
            .bind(&emp_id)
            .bind("DEMO-ADMIN-001")
            .bind("card")
            .bind("active")
            .bind(&now)
            .execute(pool)
            .await?;
        }
    }

    let zone_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO zones (id, site_id, name, risk_level, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&zone_id)
    .bind(&site_id)
    .bind("Büro")
    .bind("normal")
    .bind(&now)
    .execute(pool)
    .await?;

    let door_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO doors (id, site_id, zone_id, name, direction, status, reader_in_id, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&door_id)
    .bind(&site_id)
    .bind(&zone_id)
    .bind("Haupteingang")
    .bind("in")
    .bind("closed")
        .bind("sim.reader.main")
        .bind(&now)
        .execute(pool)
        .await?;

    sqlx::query(
        "UPDATE doors SET reader_out_id = ? WHERE id = ?",
    )
    .bind("sim.reader.main.out")
    .bind(&door_id)
    .execute(pool)
    .await?;

    if let Some(admin_emp_id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM employees WHERE employee_no = '0001' LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    {
        sqlx::query(
            r#"
            INSERT INTO access_rules (
                id, principal_type, principal_id, zone_id, door_id, schedule_json,
                valid_from, valid_to, mode, created_at
            ) VALUES (?, 'employee', ?, ?, NULL, NULL, ?, NULL, 'allow', ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&admin_emp_id)
        .bind(&zone_id)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    let policy_id = Uuid::new_v4().to_string();
    let rules = serde_json::json!({
        "jurisdiction": "DE",
        "max_daily_minutes": 600,
        "max_weekly_minutes": 2880,
        "min_break_minutes_after_6h": 30,
        "description": "ArbZG-orientiertes Basisregelwerk (v0)"
    });
    sqlx::query(
        "INSERT INTO policy_packs (id, name, jurisdiction, version, rules_json, active, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&policy_id)
    .bind("Deutschland Basis")
    .bind("DE")
    .bind("0.1.0")
    .bind(rules.to_string())
    .bind(1)
    .bind(&now)
    .execute(pool)
    .await?;

    info!(
        username = DEFAULT_ADMIN_USER,
        "default admin created — change password after first login"
    );
    Ok(())
}

/// Built-in development credentials (must not be used when demo seeding is off).
pub fn is_known_default_credential(username: &str, password: &str) -> bool {
    matches!(
        (username, password),
        ("admin", "admin") | ("demo", "demo") | ("manager", "demo")
    )
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("yes") | Ok("TRUE") | Ok("Yes")
    )
}

/// Whether demo users and week seed run on startup (default: true).
pub fn is_demo_seeding_enabled() -> bool {
    !env_flag("TIMESHARDS_DISABLE_DEMO")
}

/// When true, built-in passwords are rejected at login even if demo seeding is on.
pub fn is_block_default_passwords_enabled() -> bool {
    env_flag("TIMESHARDS_BLOCK_DEFAULT_PASSWORDS")
}

/// Login policy for factory default username/password pairs.
pub fn is_default_password_login_blocked(username: &str, password: &str) -> bool {
    if !is_known_default_credential(username, password) {
        return false;
    }
    !is_demo_seeding_enabled() || is_block_default_passwords_enabled()
}

/// Demo logins for client/manager testing (idempotent — safe every server start).
pub async fn ensure_demo_accounts(pool: &SqlitePool) -> anyhow::Result<()> {
    ensure_work_calendar_foundation(pool).await?;

    if !is_demo_seeding_enabled() {
        tracing::info!("demo seeding skipped (TIMESHARDS_DISABLE_DEMO is set)");
        return Ok(());
    }

    let zone_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM zones WHERE name = 'Büro' LIMIT 1")
            .fetch_optional(pool)
            .await?;

    upsert_demo_user(
        pool,
        "demo",
        "demo",
        "Demo Mitarbeiter",
        "employee",
        "0002",
        Some("DEMO-0002"),
        zone_id.as_deref(),
    )
    .await?;

    upsert_demo_user(
        pool,
        "manager",
        "demo",
        "Demo Vorgesetzte/r",
        "manager",
        "0003",
        Some("DEMO-0003"),
        zone_id.as_deref(),
    )
    .await?;

    ensure_demo_badges_for_existing_employees(pool).await?;
    seed_demo_week_data(pool).await?;
    assign_all_active_employees(pool).await?;

    Ok(())
}

/// Issue demo badges / Büro rules for PN 0002 and 0003 when accounts predate badge seeding.
async fn ensure_demo_badges_for_existing_employees(pool: &SqlitePool) -> anyhow::Result<()> {
    let zone_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM zones WHERE name = 'Büro' LIMIT 1")
            .fetch_optional(pool)
            .await?;

    for (employee_no, credential_uid) in [("0002", "DEMO-0002"), ("0003", "DEMO-0003")] {
        let Some(emp_id): Option<String> = sqlx::query_scalar(
            "SELECT id FROM employees WHERE employee_no = ? LIMIT 1",
        )
        .bind(employee_no)
        .fetch_optional(pool)
        .await?
        else {
            continue;
        };

        let active_badges: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM badges WHERE employee_id = ? AND status = 'active'",
        )
        .bind(&emp_id)
        .fetch_one(pool)
        .await?;
        let now = Utc::now().to_rfc3339();
        if active_badges == 0 {
            sqlx::query(
                r#"
                INSERT INTO badges (id, employee_id, credential_uid, credential_type, status, issued_at)
                VALUES (?, ?, ?, 'card', 'active', ?)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&emp_id)
            .bind(credential_uid)
            .bind(&now)
            .execute(pool)
            .await?;
            info!(employee_no, credential_uid, "demo badge ensured");
        }

        if let Some(zid) = &zone_id {
            let rule_exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM access_rules WHERE principal_id = ? AND zone_id = ?",
            )
            .bind(&emp_id)
            .bind(zid)
            .fetch_one(pool)
            .await?;
            if rule_exists == 0 {
                sqlx::query(
                    r#"
                    INSERT INTO access_rules (
                        id, principal_type, principal_id, zone_id, door_id, schedule_json,
                        valid_from, valid_to, mode, created_at
                    ) VALUES (?, 'employee', ?, ?, NULL, NULL, ?, NULL, 'allow', ?)
                    "#,
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&emp_id)
                .bind(zid)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await?;
            }
        }
    }
    Ok(())
}

/// Mo–Fr shift templates for demo employee (idempotent).
pub async fn seed_demo_week_data(pool: &SqlitePool) -> anyhow::Result<()> {
    let demo_emp: Option<String> = sqlx::query_scalar(
        "SELECT id FROM employees WHERE employee_no = '0002' LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    let Some(emp_id) = demo_emp else {
        return Ok(());
    };

    let site_id: String = sqlx::query_scalar("SELECT id FROM sites LIMIT 1")
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no site for demo templates"))?;

    let tpl_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM shift_templates WHERE employee_id = ? AND active = 1",
    )
    .bind(&emp_id)
    .fetch_one(pool)
    .await?;
    let now = Utc::now().to_rfc3339();
    if tpl_count == 0 {
        for weekday in 1..=5 {
            sqlx::query(
                r#"
                INSERT INTO shift_templates (
                    id, employee_id, name, weekday, starts_time, ends_time, site_id, active, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&emp_id)
            .bind("Demo-Woche")
            .bind(weekday)
            .bind("08:00")
            .bind("16:00")
            .bind(&site_id)
            .bind(&now)
            .execute(pool)
            .await?;
        }
        info!(employee_no = "0002", "demo shift templates Mo-Fri 08:00-16:00 created");
    }

    seed_demo_shifts_for_week(pool, &emp_id, &site_id).await?;
    seed_demo_punches(pool, &emp_id).await?;
    seed_demo_timesheet(pool, &emp_id).await?;
    seed_demo_pending_absence(pool, &emp_id).await?;
    seed_demo_pending_timesheet_for_manager(pool).await?;
    Ok(())
}

/// Pending timesheet for admin employee so manager Freigaben has something to approve.
async fn seed_demo_pending_timesheet_for_manager(pool: &SqlitePool) -> anyhow::Result<()> {
    let admin_emp: Option<String> = sqlx::query_scalar(
        "SELECT id FROM employees WHERE employee_no = '0001' LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    let Some(emp_id) = admin_emp else {
        return Ok(());
    };

    let (week_start, week_end) = week_bounds_utc(Utc::now());
    let ps = week_start.to_rfc3339();
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM timesheets WHERE employee_id = ? AND period_start = ? AND status = 'pending'",
    )
    .bind(&emp_id)
    .bind(&ps)
    .fetch_one(pool)
    .await?;
    if pending > 0 {
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();
    let pe = week_end.to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO timesheets (
            id, employee_id, period_start, period_end, worked_minutes, expected_minutes, balance_minutes, overtime_minutes, status, created_at
        ) VALUES (?, ?, ?, ?, 480, 2400, -1920, 0, 'pending', ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&emp_id)
    .bind(&ps)
    .bind(&pe)
    .bind(&now)
    .execute(pool)
    .await?;
    info!("demo pending timesheet for admin (manager approval flow)");
    Ok(())
}

async fn seed_demo_timesheet(pool: &SqlitePool, emp_id: &str) -> anyhow::Result<()> {
    let (week_start, week_end) = week_bounds_utc(Utc::now());
    if upsert_draft_timesheet_for_week(pool, emp_id, week_start).await? {
        info!(employee_no = "0002", "demo draft timesheet for current week");
        return Ok(());
    }

    let ps = week_start.to_rfc3339();
    let pe = week_end.to_rfc3339();
    let existing: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM timesheets
        WHERE employee_id = ? AND period_start = ?
          AND status IN ('draft', 'rejected', 'pending')
        "#,
    )
    .bind(emp_id)
    .bind(&ps)
    .fetch_one(pool)
    .await?;
    if existing > 0 {
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO timesheets (
            id, employee_id, period_start, period_end, worked_minutes, expected_minutes, balance_minutes, overtime_minutes, status, created_at
        ) VALUES (?, ?, ?, ?, 480, 2400, -1920, 0, 'draft', ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(emp_id)
    .bind(&ps)
    .bind(&pe)
    .bind(&now)
    .execute(pool)
    .await?;
    info!(employee_no = "0002", "demo draft timesheet (fallback) for current week");
    Ok(())
}

async fn seed_demo_pending_absence(pool: &SqlitePool, emp_id: &str) -> anyhow::Result<()> {
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM absence_requests WHERE employee_id = ? AND status = 'pending'",
    )
    .bind(emp_id)
    .fetch_one(pool)
    .await?;
    if pending > 0 {
        return Ok(());
    }

    let now = Utc::now();
    let starts = (now + Duration::days(14)).to_rfc3339();
    let ends = (now + Duration::days(21)).to_rfc3339();
    let created = now.to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO absence_requests (
            id, employee_id, absence_type, starts_at, ends_at, status, reason, created_at
        ) VALUES (?, ?, 'urlaub', ?, ?, 'pending', 'Demo-Antrag', ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(emp_id)
    .bind(&starts)
    .bind(&ends)
    .bind(&created)
    .execute(pool)
    .await?;
    info!("demo pending absence for manager approval flow");
    Ok(())
}

fn shift_bounds_berlin(
    week_start: chrono::DateTime<Utc>,
    weekday: i32,
    starts_time: &str,
    ends_time: &str,
) -> Option<(String, String)> {
    let start_hm = parse_hm(starts_time)?;
    let end_hm = parse_hm(ends_time)?;
    let monday = week_start.date_naive();
    let day = monday + Duration::days((weekday - 1) as i64);
    let tz: Tz = chrono_tz::Europe::Berlin;
    let start_naive = day.and_hms_opt(start_hm.hour(), start_hm.minute(), 0)?;
    let end_naive = day.and_hms_opt(end_hm.hour(), end_hm.minute(), 0)?;
    let start_local = start_naive.and_local_timezone(tz).single()?;
    let end_local = end_naive.and_local_timezone(tz).single()?;
    Some((start_local.to_rfc3339(), end_local.to_rfc3339()))
}

fn parse_hm(s: &str) -> Option<NaiveTime> {
    let mut p = s.split(':');
    let h: u32 = p.next()?.parse().ok()?;
    let m: u32 = p.next()?.parse().ok()?;
    NaiveTime::from_hms_opt(h, m, 0)
}

async fn seed_demo_shifts_for_week(
    pool: &SqlitePool,
    emp_id: &str,
    site_id: &str,
) -> anyhow::Result<()> {
    let (week_start, week_end) = {
        let (start, end) = week_bounds_utc(Utc::now());
        (start.to_rfc3339(), end.to_rfc3339())
    };
    let existing: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM shift_instances
        WHERE employee_id = ? AND status != 'cancelled'
          AND starts_at >= ? AND starts_at < ?
        "#,
    )
    .bind(emp_id)
    .bind(&week_start)
    .bind(&week_end)
    .fetch_one(pool)
    .await?;
    if existing > 0 {
        return Ok(());
    }

    let (week_start_dt, _) = week_bounds_utc(Utc::now());
    let now = Utc::now().to_rfc3339();
    let mut created = 0u32;
    for weekday in 1..=5 {
        let Some((starts_at, ends_at)) = shift_bounds_berlin(week_start_dt, weekday, "08:00", "16:00")
        else {
            continue;
        };
        sqlx::query(
            r#"
            INSERT INTO shift_instances (id, employee_id, site_id, starts_at, ends_at, status, created_at)
            VALUES (?, ?, ?, ?, ?, 'published', ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(emp_id)
        .bind(site_id)
        .bind(&starts_at)
        .bind(&ends_at)
        .bind(&now)
        .execute(pool)
        .await?;
        created += 1;
    }
    if created > 0 {
        info!(created, "demo planned shifts for current week");
    }
    Ok(())
}

async fn seed_demo_punches(pool: &SqlitePool, emp_id: &str) -> anyhow::Result<()> {
    let punch_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM time_events WHERE employee_id = ?")
            .bind(emp_id)
            .fetch_one(pool)
            .await?;
    if punch_count > 0 {
        return Ok(());
    }

    let yesterday = Utc::now().date_naive() - Duration::days(1);
    let tz: Tz = chrono_tz::Europe::Berlin;
    let Some(clock_in_dt) = yesterday
        .and_hms_opt(8, 5, 0)
        .and_then(|t| t.and_local_timezone(tz).single())
    else {
        return Ok(());
    };
    let Some(clock_out_dt) = yesterday
        .and_hms_opt(16, 2, 0)
        .and_then(|t| t.and_local_timezone(tz).single())
    else {
        return Ok(());
    };
    let clock_in = clock_in_dt.to_rfc3339();
    let clock_out = clock_out_dt.to_rfc3339();

    let now = Utc::now().to_rfc3339();
    for (kind, at) in [("clock_in", clock_in), ("clock_out", clock_out)] {
        sqlx::query(
            r#"
            INSERT INTO time_events (id, employee_id, kind, occurred_at, source, notes, created_at)
            VALUES (?, ?, ?, ?, 'seed', NULL, ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(emp_id)
        .bind(kind)
        .bind(&at)
        .bind(&now)
        .execute(pool)
        .await?;
    }
    info!(employee_no = "0002", "demo punches for prior day created");
    Ok(())
}

async fn upsert_demo_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    display_name: &str,
    role_key: &str,
    employee_no: &str,
    badge_uid: Option<&str>,
    zone_id: Option<&str>,
) -> anyhow::Result<()> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = ?")
        .bind(username)
        .fetch_one(pool)
        .await?;
    if exists > 0 {
        return Ok(());
    }

    let role_id: String = sqlx::query_scalar("SELECT id FROM roles WHERE template_key = ? LIMIT 1")
        .bind(role_key)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("role {role_key} missing"))?;

    let now = Utc::now().to_rfc3339();
    let user_id = Uuid::new_v4().to_string();
    let hash = hash_password(password)?;
    sqlx::query(
        r#"
        INSERT INTO users (id, username, display_name, email, password_hash, locale, status, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&user_id)
    .bind(username)
    .bind(display_name)
    .bind(format!("{username}@local"))
    .bind(&hash)
    .bind("de")
    .bind("active")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(&user_id)
        .bind(&role_id)
        .execute(pool)
        .await?;

    let emp_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO employees (id, user_id, employee_no, display_name, active_from, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&emp_id)
    .bind(&user_id)
    .bind(employee_no)
    .bind(display_name)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    if let Some(uid) = badge_uid {
        sqlx::query(
            r#"
            INSERT INTO badges (id, employee_id, credential_uid, credential_type, status, issued_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&emp_id)
        .bind(uid)
        .bind("card")
        .bind("active")
        .bind(&now)
        .execute(pool)
        .await?;
    }

    if let Some(zid) = zone_id {
        let rule_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM access_rules WHERE principal_id = ? AND zone_id = ?",
        )
        .bind(&emp_id)
        .bind(zid)
        .fetch_one(pool)
        .await?;
        if rule_exists == 0 {
            sqlx::query(
                r#"
                INSERT INTO access_rules (
                    id, principal_type, principal_id, zone_id, door_id, schedule_json,
                    valid_from, valid_to, mode, created_at
                ) VALUES (?, 'employee', ?, ?, NULL, NULL, ?, NULL, 'allow', ?)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&emp_id)
            .bind(zid)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await?;
        }
    }

    info!(username, employee_no, "demo account created");
    Ok(())
}

/// Keeps built-in role permission sets in sync (safe to run every server start).
pub async fn sync_role_permissions(pool: &SqlitePool) -> anyhow::Result<()> {
    for template in [
        RoleTemplate::SystemAdmin,
        RoleTemplate::HrAdmin,
        RoleTemplate::SecurityOperator,
        RoleTemplate::Manager,
        RoleTemplate::Employee,
    ] {
        let key = template_key(template);
        let perms: Vec<String> = template.permissions().keys().cloned().collect();
        let perms_json = serde_json::to_string(&perms)?;
        sqlx::query("UPDATE roles SET permissions_json = ? WHERE template_key = ?")
            .bind(&perms_json)
            .bind(key)
            .execute(pool)
            .await?;
    }
    Ok(())
}

fn template_key(template: RoleTemplate) -> &'static str {
    match template {
        RoleTemplate::SystemAdmin => "system_admin",
        RoleTemplate::HrAdmin => "hr_admin",
        RoleTemplate::SecurityOperator => "security_operator",
        RoleTemplate::Manager => "manager",
        RoleTemplate::Employee => "employee",
    }
}

