CREATE TABLE IF NOT EXISTS user_contexts (
    context_id TEXT PRIMARY KEY NOT NULL,

    user_id TEXT NOT NULL,

    session_id TEXT,

    name TEXT NOT NULL,

    kind TEXT NOT NULL DEFAULT 'user',

    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT fk_user_contexts_session
        FOREIGN KEY (session_id)
        REFERENCES user_sessions(session_id)
        ON DELETE SET NULL,
    CONSTRAINT fk_user_contexts_user
        FOREIGN KEY (user_id)
        REFERENCES users(id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_user_contexts_user ON user_contexts(user_id);
CREATE INDEX IF NOT EXISTS idx_user_contexts_user_updated ON user_contexts(user_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_user_contexts_session ON user_contexts(session_id);

CREATE OR REPLACE TRIGGER update_user_contexts_updated_at
    BEFORE UPDATE ON user_contexts
    FOR EACH ROW
    EXECUTE FUNCTION update_timestamp_trigger();
