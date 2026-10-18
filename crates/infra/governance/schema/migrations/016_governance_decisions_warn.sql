-- Warn mode: a policy configured `mode: warn` records the finding it would
-- have denied on and lets the call through. The row is a real decision with a
-- real reason, so it lands in this table like any other; only the verdict is
-- new. The CHECK constraint is rebuilt rather than extended because Postgres
-- has no ALTER for a check expression.
ALTER TABLE governance_decisions DROP CONSTRAINT IF EXISTS governance_decisions_decision_check;
ALTER TABLE governance_decisions
    ADD CONSTRAINT governance_decisions_decision_check
    CHECK (decision IN ('allow', 'warn', 'deny', 'pending'));
