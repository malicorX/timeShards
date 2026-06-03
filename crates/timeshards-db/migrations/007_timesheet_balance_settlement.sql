-- Weekly balance on timesheets; settlement rules (Wochenperiode foundation).

ALTER TABLE timesheets ADD COLUMN balance_minutes INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS settlement_rules (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    period_kind TEXT NOT NULL,
    config_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

ALTER TABLE work_calendars ADD COLUMN settlement_rule_id TEXT REFERENCES settlement_rules(id);
