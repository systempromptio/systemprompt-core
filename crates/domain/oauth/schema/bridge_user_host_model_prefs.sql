CREATE TABLE IF NOT EXISTS bridge_user_host_model_prefs (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    host_id TEXT NOT NULL,
    model_protocols TEXT[] NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, host_id)
);
CREATE INDEX IF NOT EXISTS idx_bridge_user_host_model_prefs_user
    ON bridge_user_host_model_prefs(user_id);
