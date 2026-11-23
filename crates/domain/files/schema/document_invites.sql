CREATE TABLE IF NOT EXISTS document_invites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    email TEXT NOT NULL CHECK (email = lower(email) AND length(email) BETWEEN 3 AND 320),
    access_level TEXT NOT NULL CHECK (access_level IN ('view', 'comment')),
    token_hash TEXT NOT NULL CHECK (length(token_hash) = 64),
    invited_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    claimed_by_user_id VARCHAR(255),
    claimed_at TIMESTAMPTZ,
    CHECK ((claimed_by_user_id IS NULL) = (claimed_at IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_invites_token
    ON document_invites (token_hash);
CREATE INDEX IF NOT EXISTS idx_document_invites_document
    ON document_invites (document_id, created_at);
CREATE INDEX IF NOT EXISTS idx_document_invites_open_email
    ON document_invites (email) WHERE claimed_at IS NULL AND revoked_at IS NULL;
