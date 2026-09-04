-- See the column comment in ai_safety_findings.sql: warn mode separates
-- "matched a block category" from "actually blocked the call". Existing rows
-- were all written by an enforcing gateway, but a finding that was merely
-- audited was never blocking either, so FALSE is the only defensible backfill
-- and the report treats pre-migration rows as unknown-but-not-blocked.
ALTER TABLE ai_safety_findings ADD COLUMN IF NOT EXISTS blocked BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX IF NOT EXISTS idx_ai_safety_findings_blocked ON ai_safety_findings(blocked);
