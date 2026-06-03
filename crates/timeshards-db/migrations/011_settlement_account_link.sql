-- Link time account entries to monthly settlement periods (reconciliation postings).

ALTER TABLE time_account_entries ADD COLUMN settlement_period_id TEXT REFERENCES settlement_periods(id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_time_account_entries_settlement
    ON time_account_entries(settlement_period_id, account_kind)
    WHERE settlement_period_id IS NOT NULL;
