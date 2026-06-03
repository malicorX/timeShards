-- AI TimeShards initial schema (SQLite; Postgres-compatible types via sqlx)

CREATE TABLE IF NOT EXISTS sites (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'Europe/Berlin',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    email TEXT,
    password_hash TEXT NOT NULL,
    locale TEXT NOT NULL DEFAULT 'de',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS roles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    template_key TEXT,
    permissions_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_roles (
    user_id TEXT NOT NULL REFERENCES users(id),
    role_id TEXT NOT NULL REFERENCES roles(id),
    PRIMARY KEY (user_id, role_id)
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id),
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS employees (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT REFERENCES users(id),
    employee_no TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    org_unit TEXT,
    manager_id TEXT,
    active_from TEXT NOT NULL,
    active_to TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS badges (
    id TEXT PRIMARY KEY NOT NULL,
    employee_id TEXT REFERENCES employees(id),
    credential_uid TEXT NOT NULL UNIQUE,
    credential_type TEXT NOT NULL DEFAULT 'card',
    status TEXT NOT NULL DEFAULT 'active',
    issued_at TEXT NOT NULL,
    revoked_at TEXT
);

CREATE TABLE IF NOT EXISTS zones (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id),
    name TEXT NOT NULL,
    parent_zone_id TEXT,
    risk_level TEXT NOT NULL DEFAULT 'normal',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS doors (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id),
    zone_id TEXT REFERENCES zones(id),
    name TEXT NOT NULL,
    direction TEXT NOT NULL DEFAULT 'in',
    status TEXT NOT NULL DEFAULT 'closed',
    reader_in_id TEXT,
    reader_out_id TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS access_rules (
    id TEXT PRIMARY KEY NOT NULL,
    principal_type TEXT NOT NULL,
    principal_id TEXT NOT NULL,
    zone_id TEXT REFERENCES zones(id),
    door_id TEXT REFERENCES doors(id),
    schedule_json TEXT,
    valid_from TEXT NOT NULL,
    valid_to TEXT,
    mode TEXT NOT NULL DEFAULT 'allow',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS access_events (
    id TEXT PRIMARY KEY NOT NULL,
    badge_id TEXT,
    employee_id TEXT,
    door_id TEXT,
    zone_id TEXT,
    decision TEXT NOT NULL,
    reason_code TEXT,
    occurred_at TEXT NOT NULL,
    correlation_id TEXT,
    raw_payload_json TEXT
);

CREATE TABLE IF NOT EXISTS schedules (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'Europe/Berlin',
    rule_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS shift_instances (
    id TEXT PRIMARY KEY NOT NULL,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    site_id TEXT NOT NULL REFERENCES sites(id),
    starts_at TEXT NOT NULL,
    ends_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'planned',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS time_events (
    id TEXT PRIMARY KEY NOT NULL,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    kind TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'manual',
    notes TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS timesheets (
    id TEXT PRIMARY KEY NOT NULL,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    worked_minutes INTEGER NOT NULL DEFAULT 0,
    overtime_minutes INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS domain_events (
    id TEXT PRIMARY KEY NOT NULL,
    topic TEXT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    occurred_at TEXT NOT NULL,
    producer TEXT NOT NULL,
    correlation_id TEXT,
    payload_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id TEXT,
    action TEXT NOT NULL,
    object_type TEXT NOT NULL,
    object_id TEXT,
    occurred_at TEXT NOT NULL,
    reason TEXT,
    before_json TEXT,
    after_json TEXT,
    hash_prev TEXT,
    hash_self TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS policy_packs (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    jurisdiction TEXT NOT NULL DEFAULT 'DE',
    version TEXT NOT NULL,
    rules_json TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_time_events_employee ON time_events(employee_id, occurred_at);
CREATE INDEX IF NOT EXISTS idx_access_events_occurred ON access_events(occurred_at);
CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash);
