ALTER TABLE access_control_entities
    DROP COLUMN IF EXISTS reason,
    DROP COLUMN IF EXISTS changed_at,
    DROP COLUMN IF EXISTS changed_by;
ALTER TABLE access_control_rules
    DROP COLUMN IF EXISTS reason,
    DROP COLUMN IF EXISTS changed_at,
    DROP COLUMN IF EXISTS changed_by;
