CREATE TABLE IF NOT EXISTS document_collab_updates (
    session_id UUID NOT NULL REFERENCES document_edit_sessions(id) ON DELETE CASCADE,
    seq BIGSERIAL NOT NULL,
    payload BYTEA NOT NULL,
    author_user_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    actor_kind TEXT CONSTRAINT document_collab_updates_actor_kind_check
        CHECK (actor_kind IS NULL OR actor_kind IN ('human', 'agent', 'system')),
    actor_name TEXT,
    actor_attended BOOLEAN,
    edit_level TEXT CONSTRAINT document_collab_updates_edit_level_check
        CHECK (edit_level IS NULL OR edit_level IN ('edit', 'comment')),
    PRIMARY KEY (session_id, seq)
);
