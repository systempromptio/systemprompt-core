CREATE TABLE IF NOT EXISTS context_agents (
    id SERIAL PRIMARY KEY,

    context_id TEXT NOT NULL,

    agent_name TEXT NOT NULL,

    added_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    last_active_at TIMESTAMPTZ,

    FOREIGN KEY (context_id) REFERENCES user_contexts(context_id) ON DELETE CASCADE,

    UNIQUE(context_id, agent_name)
);

CREATE INDEX IF NOT EXISTS idx_context_agents_context
    ON context_agents(context_id);

CREATE INDEX IF NOT EXISTS idx_context_agents_agent_name
    ON context_agents(agent_name);

CREATE INDEX IF NOT EXISTS idx_context_agents_active
    ON context_agents(context_id, last_active_at DESC);
