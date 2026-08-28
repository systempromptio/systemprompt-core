-- Calls held for a human decision by the `require_approval` governance policy.
--
-- The row is the rendezvous point between two processes: the MCP server that
-- parked the call blocks on it, and the admin console resolves it. Keyed by
-- `call_id` rather than a fresh uuid because `GovernancePolicy::evaluate` is
-- contractually idempotent per call — a retried MRTR round for the same call
-- must find the row it already created, not open a second approval.
CREATE TABLE IF NOT EXISTS approval_requests (
    call_id TEXT PRIMARY KEY,
    tool_name TEXT NOT NULL,
    server_name TEXT NOT NULL,
    -- The exact arguments the approver is authorising. `args_digest` binds the
    -- decision to them: a retry that changes the payload after approval no
    -- longer matches and is re-held rather than silently executing.
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
    -- An approver, a timestamp and a note only make sense once a decision has
    -- been taken; a 'pending' row carrying them would be unexplainable.
    CONSTRAINT approval_decided_fields CHECK (
        (status = 'pending' AND approver_id IS NULL AND decided_at IS NULL)
        OR (status <> 'pending')
    )
);

CREATE INDEX IF NOT EXISTS idx_approval_requests_status ON approval_requests(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_approval_requests_requester ON approval_requests(requested_by);
CREATE INDEX IF NOT EXISTS idx_approval_requests_trace ON approval_requests(trace_id);
