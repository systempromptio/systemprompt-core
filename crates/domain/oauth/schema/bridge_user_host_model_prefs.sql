CREATE TABLE IF NOT EXISTS bridge_user_host_model_prefs (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    host_id TEXT NOT NULL
        CONSTRAINT bridge_user_host_model_prefs_host_id_check
        CHECK (host_id IN ('claude-code', 'claude-desktop', 'codex', 'codex-cli', 'opencode', 'opencode-cli', 'hermes')),
    model_protocols TEXT[] NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, host_id)
);
