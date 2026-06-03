CREATE TABLE IF NOT EXISTS shift_templates (
    id TEXT PRIMARY KEY NOT NULL,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    name TEXT NOT NULL,
    weekday INTEGER NOT NULL,
    starts_time TEXT NOT NULL,
    ends_time TEXT NOT NULL,
    site_id TEXT REFERENCES sites(id),
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_shift_templates_employee ON shift_templates(employee_id, weekday);
