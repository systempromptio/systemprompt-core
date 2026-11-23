CREATE TABLE IF NOT EXISTS document_public_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL CHECK (length(token_hash) = 64),
    password_hash TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    open_count BIGINT NOT NULL DEFAULT 0,
    last_opened_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_public_links_token
    ON document_public_links (token_hash);
CREATE INDEX IF NOT EXISTS idx_document_public_links_document
    ON document_public_links (document_id, created_at);
