-- Work calendar foundation: workday models, calendars, employee assignments.
-- Replaces shift_templates as the source of truth for expected work time (templates kept for legacy UI).

CREATE TABLE IF NOT EXISTS workday_models (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    config_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS holiday_calendars (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    region_code TEXT,
    year_from INTEGER NOT NULL,
    year_to INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS holiday_calendar_days (
    calendar_id TEXT NOT NULL REFERENCES holiday_calendars(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    day_kind TEXT NOT NULL,
    name TEXT,
    workday_model_id TEXT REFERENCES workday_models(id),
    PRIMARY KEY (calendar_id, date)
);

CREATE INDEX IF NOT EXISTS idx_holiday_calendar_days_date ON holiday_calendar_days(date);

CREATE TABLE IF NOT EXISTS work_calendars (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    holiday_calendar_id TEXT REFERENCES holiday_calendars(id),
    week_close_weekday INTEGER NOT NULL DEFAULT 6,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS work_calendar_days (
    calendar_id TEXT NOT NULL REFERENCES work_calendars(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    workday_model_id TEXT NOT NULL REFERENCES workday_models(id),
    PRIMARY KEY (calendar_id, date)
);

CREATE INDEX IF NOT EXISTS idx_work_calendar_days_cal_date ON work_calendar_days(calendar_id, date);

CREATE TABLE IF NOT EXISTS employee_work_assignments (
    id TEXT PRIMARY KEY NOT NULL,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    work_calendar_id TEXT NOT NULL REFERENCES work_calendars(id),
    valid_from TEXT NOT NULL,
    valid_to TEXT,
    part_time_percent INTEGER NOT NULL DEFAULT 100,
    notes TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_employee_work_assign_emp ON employee_work_assignments(employee_id, valid_from);

ALTER TABLE timesheets ADD COLUMN expected_minutes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE timesheets ADD COLUMN evaluation_json TEXT;
