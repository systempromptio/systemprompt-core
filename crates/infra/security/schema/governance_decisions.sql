CREATE TABLE IF NOT EXISTS governance_decisions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    agent_id TEXT,
    agent_scope TEXT,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny', 'pending')),
    policy TEXT NOT NULL,
    reason TEXT NOT NULL,
    evaluated_rules JSONB DEFAULT '[]',
    plugin_id TEXT,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'anonymous', 'system', 'job', 'mcp', 'agent')),
    actor_id TEXT NOT NULL CHECK (length(actor_id) > 0),
    act_chain JSONB NOT NULL DEFAULT '[]'::jsonb,
    context_id TEXT NOT NULL,
    task_id TEXT,
    -- The request-plane correlator. Enforcement sites without a session (MCP,
    -- server-attach RBAC) previously borrowed session_id to carry this, which
    -- made the trace join depend on an overloaded column; it is its own key now.
    trace_id TEXT,
    client_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_governance_decisions_act_chain ON governance_decisions USING GIN (act_chain);

CREATE INDEX IF NOT EXISTS idx_governance_decisions_user ON governance_decisions(user_id);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_session ON governance_decisions(session_id);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_decision ON governance_decisions(decision);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_created ON governance_decisions(created_at);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_rate_limit ON governance_decisions(session_id, user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_actor ON governance_decisions(actor_kind, actor_id);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_context ON governance_decisions(context_id);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_trace ON governance_decisions(trace_id);

-- Adopted from the astound web extension, which created these out-of-tree
-- against a core table because the per-policy rollups and windowed rankings
-- that need them live downstream. Declared here so a fresh database gets them
-- with the table rather than on a later extension migration run.
CREATE INDEX IF NOT EXISTS idx_governance_decisions_policy_created ON governance_decisions(policy, created_at);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_tool_name ON governance_decisions(tool_name);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_client ON governance_decisions(client_id);
