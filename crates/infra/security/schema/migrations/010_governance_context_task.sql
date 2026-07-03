ALTER TABLE governance_decisions ADD COLUMN IF NOT EXISTS context_id TEXT;
ALTER TABLE governance_decisions ADD COLUMN IF NOT EXISTS task_id TEXT;

CREATE INDEX IF NOT EXISTS idx_governance_decisions_context ON governance_decisions(context_id);
