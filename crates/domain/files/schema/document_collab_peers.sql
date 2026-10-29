CREATE TABLE IF NOT EXISTS document_collab_peers (
    session_id UUID NOT NULL REFERENCES document_edit_sessions(id) ON DELETE CASCADE,
    peer_id BIGINT NOT NULL CHECK (peer_id > 0),
    user_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (session_id, peer_id)
);

CREATE INDEX IF NOT EXISTS idx_document_collab_peers_user
    ON document_collab_peers(session_id, user_id);
