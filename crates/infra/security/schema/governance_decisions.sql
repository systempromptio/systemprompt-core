CREATE TABLE IF NOT EXISTS governance_decisions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    agent_id TEXT,
    agent_scope TEXT,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny')),
    policy TEXT NOT NULL,
    reason TEXT NOT NULL,
    evaluated_rules JSONB DEFAULT '[]',
    plugin_id TEXT,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'anonymous', 'system', 'job', 'mcp', 'agent')),
    actor_id TEXT NOT NULL CHECK (length(actor_id) > 0),
    act_chain JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_governance_decisions_act_chain ON governance_decisions USING GIN (act_chain);

CREATE INDEX IF NOT EXISTS idx_governance_decisions_user ON governance_decisions(user_id);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_session ON governance_decisions(session_id);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_decision ON governance_decisions(decision);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_created ON governance_decisions(created_at);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_rate_limit ON governance_decisions(session_id, user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_governance_decisions_actor ON governance_decisions(actor_kind, actor_id);
