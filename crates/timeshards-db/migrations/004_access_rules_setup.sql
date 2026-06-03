-- Exit reader for simulator anti-passback demo
UPDATE doors SET reader_out_id = 'sim.reader.main.out'
WHERE reader_in_id = 'sim.reader.main' AND reader_out_id IS NULL;

-- Default allow rule for seeded admin employee on Büro zone (if none exist)
INSERT INTO access_rules (
    id, principal_type, principal_id, zone_id, door_id, schedule_json,
    valid_from, valid_to, mode, created_at
)
SELECT
    lower(hex(randomblob(16))),
    'employee',
    e.id,
    z.id,
    NULL,
    NULL,
    datetime('now'),
    NULL,
    'allow',
    datetime('now')
FROM employees e
CROSS JOIN zones z
WHERE e.employee_no = '0001'
  AND z.name = 'Büro'
  AND NOT EXISTS (SELECT 1 FROM access_rules LIMIT 1);
