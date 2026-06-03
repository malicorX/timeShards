-- Shift rotation / Umschaltplan: cyclic workday models override calendar days (holidays still win).

CREATE TABLE IF NOT EXISTS work_rotation_plans (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    anchor_date TEXT NOT NULL,
    cycle_days INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS work_rotation_slots (
    plan_id TEXT NOT NULL REFERENCES work_rotation_plans(id) ON DELETE CASCADE,
    slot_index INTEGER NOT NULL,
    workday_model_id TEXT NOT NULL REFERENCES workday_models(id),
    PRIMARY KEY (plan_id, slot_index)
);

ALTER TABLE work_calendars ADD COLUMN rotation_plan_id TEXT REFERENCES work_rotation_plans(id);
