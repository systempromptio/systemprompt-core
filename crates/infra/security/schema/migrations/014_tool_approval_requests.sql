-- Third governance outcome: held for a human.
--
-- `Decision` gained a `Pending` variant, and `DecisionTag` is sqlx-bound to
-- this column — the type doc is explicit that adding a variant without
-- extending the constraint fails the build, so the two move together. The
-- constraint is unnamed in the original CREATE TABLE, so it is located
-- through the catalog rather than dropped by a guessed name.
DO $$
DECLARE
    constraint_name TEXT;
BEGIN
    SELECT con.conname INTO constraint_name
    FROM pg_constraint con
    JOIN pg_class rel ON rel.oid = con.conrelid
    WHERE rel.relname = 'governance_decisions'
      AND con.contype = 'c'
      AND pg_get_constraintdef(con.oid) LIKE '%decision%'
      AND pg_get_constraintdef(con.oid) NOT LIKE '%actor_kind%';

    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE governance_decisions DROP CONSTRAINT %I', constraint_name);
    END IF;
END $$;

ALTER TABLE governance_decisions
    ADD CONSTRAINT governance_decisions_decision_check
    CHECK (decision IN ('allow', 'deny', 'pending'));

-- See schema/approval_requests.sql for why this is keyed by call_id.
CREATE TABLE IF NOT EXISTS approval_requests (
    call_id TEXT PRIMARY KEY,
    tool_name TEXT NOT NULL,
    server_name TEXT NOT NULL,
    arguments JSONB NOT NULL DEFAULT '{}'::jsonb,
    args_digest TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    session_id TEXT,
    trace_id TEXT,
    rule TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'denied', 'expired')),
    approver_id TEXT,
    approver_username TEXT,
    decided_at TIMESTAMPTZ,
    decision_note TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT approval_decided_fields CHECK (
        (status = 'pending' AND approver_id IS NULL AND decided_at IS NULL)
        OR (status <> 'pending')
    )
);

CREATE INDEX IF NOT EXISTS idx_approval_requests_status ON approval_requests(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_approval_requests_requester ON approval_requests(requested_by);
CREATE INDEX IF NOT EXISTS idx_approval_requests_trace ON approval_requests(trace_id);
