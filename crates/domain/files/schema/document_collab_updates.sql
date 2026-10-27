CREATE TABLE IF NOT EXISTS document_collab_updates (
    session_id UUID NOT NULL REFERENCES document_edit_sessions(id) ON DELETE CASCADE,
    seq BIGSERIAL NOT NULL,
    update BYTEA NOT NULL,
    author_user_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (session_id, seq)
);
