CREATE TABLE IF NOT EXISTS document_link_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    link_id UUID NOT NULL REFERENCES document_public_links(id) ON DELETE CASCADE,
    client_ip TEXT NOT NULL,
    attempted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_link_attempts_window
    ON document_link_attempts (link_id, client_ip, attempted_at);
