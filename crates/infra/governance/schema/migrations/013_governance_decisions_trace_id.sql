-- Promote the trace correlator to its own column.
--
-- Enforcement sites without a session wrote the trace id into session_id so
-- the trace explorer had something to join on. That made a session id which
-- happened to look like a trace id join the wrong rows, with no constraint
-- against it. The id was always also present in the evaluated_rules audit
-- blob, so history backfills exactly rather than being lost.

ALTER TABLE governance_decisions ADD COLUMN IF NOT EXISTS trace_id TEXT;

UPDATE governance_decisions
SET trace_id = evaluated_rules->>'trace_id'
WHERE trace_id IS NULL
  AND evaluated_rules->>'trace_id' IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_governance_decisions_trace ON governance_decisions(trace_id);

-- Adopted from extensions/web/schema/migrations/029_analytics_indexes.sql,
-- which created them on this core-owned table from downstream.
CREATE INDEX IF NOT EXISTS idx_governance_decisions_policy_created ON governance_decisions(policy, created_at);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_tool_name ON governance_decisions(tool_name);
