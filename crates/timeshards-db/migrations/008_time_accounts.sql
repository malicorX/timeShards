-- Time accounts (Konten): cumulative balances posted on timesheet approval.

CREATE TABLE IF NOT EXISTS time_accounts (
    employee_id TEXT NOT NULL REFERENCES employees(id),
    account_kind TEXT NOT NULL,
    balance_minutes INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (employee_id, account_kind)
);

CREATE TABLE IF NOT EXISTS time_account_entries (
    id TEXT PRIMARY KEY NOT NULL,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    account_kind TEXT NOT NULL,
    timesheet_id TEXT REFERENCES timesheets(id),
    period_start TEXT,
    delta_minutes INTEGER NOT NULL,
    note TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_time_account_entries_emp ON time_account_entries(employee_id, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_time_account_entries_timesheet ON time_account_entries(timesheet_id, account_kind)
    WHERE timesheet_id IS NOT NULL;
