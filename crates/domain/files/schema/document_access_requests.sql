CREATE TABLE IF NOT EXISTS document_access_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    requester_user_id VARCHAR(255) NOT NULL,
    access_level TEXT NOT NULL CHECK (access_level IN ('view', 'comment', 'edit', 'share')),
    message TEXT,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'granted', 'declined')),
    decided_by VARCHAR(255),
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((status = 'open') = (decided_at IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_access_requests_open
    ON document_access_requests (document_id, requester_user_id) WHERE status = 'open';
CREATE INDEX IF NOT EXISTS idx_document_access_requests_document
    ON document_access_requests (document_id, created_at);
