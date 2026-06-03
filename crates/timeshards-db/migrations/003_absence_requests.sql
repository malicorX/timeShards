CREATE TABLE IF NOT EXISTS absence_requests (
    id TEXT PRIMARY KEY NOT NULL,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    absence_type TEXT NOT NULL,
    starts_at TEXT NOT NULL,
    ends_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    reason TEXT,
    decided_by TEXT,
    decided_at TEXT,
    decision_note TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_absence_employee ON absence_requests(employee_id, starts_at);
CREATE INDEX IF NOT EXISTS idx_absence_status ON absence_requests(status);
