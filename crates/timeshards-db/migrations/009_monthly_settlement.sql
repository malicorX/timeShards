-- Monthly settlement periods (Monatsperiode): snapshot after close.

CREATE TABLE IF NOT EXISTS settlement_periods (
    id TEXT PRIMARY KEY NOT NULL,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    period_kind TEXT NOT NULL DEFAULT 'month',
    year INTEGER NOT NULL,
    month INTEGER NOT NULL,
    status TEXT NOT NULL,
    worked_minutes INTEGER NOT NULL DEFAULT 0,
    expected_minutes INTEGER NOT NULL DEFAULT 0,
    balance_minutes INTEGER NOT NULL DEFAULT 0,
    overtime_minutes INTEGER NOT NULL DEFAULT 0,
    weeks_count INTEGER NOT NULL DEFAULT 0,
    summary_json TEXT,
    closed_at TEXT,
    closed_by_user_id TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(employee_id, period_kind, year, month)
);

CREATE INDEX IF NOT EXISTS idx_settlement_periods_emp
    ON settlement_periods(employee_id, year DESC, month DESC);
