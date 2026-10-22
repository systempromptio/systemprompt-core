ALTER TABLE ai_gateway_policies DROP CONSTRAINT IF EXISTS ai_gateway_policies_source_check;
ALTER TABLE ai_gateway_policies
    DROP COLUMN IF EXISTS reason,
    DROP COLUMN IF EXISTS changed_at,
    DROP COLUMN IF EXISTS changed_by,
    DROP COLUMN IF EXISTS source;
