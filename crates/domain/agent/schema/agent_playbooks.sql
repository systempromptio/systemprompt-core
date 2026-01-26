CREATE TABLE IF NOT EXISTS agent_playbooks (
    playbook_id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL UNIQUE,

    name TEXT NOT NULL,
    description TEXT NOT NULL,
    instructions TEXT NOT NULL,

    enabled BOOLEAN NOT NULL DEFAULT true,

    tags TEXT[],

    category TEXT NOT NULL,
    domain TEXT NOT NULL,
    source_id TEXT NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_agent_playbooks_enabled ON agent_playbooks(enabled);
CREATE INDEX IF NOT EXISTS idx_agent_playbooks_source ON agent_playbooks(source_id);
CREATE INDEX IF NOT EXISTS idx_agent_playbooks_category ON agent_playbooks(category);
CREATE INDEX IF NOT EXISTS idx_agent_playbooks_domain ON agent_playbooks(domain);
