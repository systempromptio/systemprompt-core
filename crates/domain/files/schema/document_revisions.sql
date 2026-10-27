CREATE TABLE IF NOT EXISTS document_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    parent_revision_id UUID REFERENCES document_revisions(id) ON DELETE SET NULL,
    content_sha256 TEXT NOT NULL REFERENCES document_blobs(content_sha256) ON DELETE RESTRICT,
    author_user_id VARCHAR(255) NOT NULL,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'bridge', 'import', 'system')),
    origin_kind TEXT NOT NULL CHECK (origin_kind IN ('mcp_execution', 'bridge_device', 'web_session', 'edit_session', 'api')),
    origin_id TEXT,
    message TEXT,
    edit_ops JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_revisions_document_created
    ON document_revisions (document_id, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_document_revisions_created
    ON document_revisions (created_at, id);
CREATE INDEX IF NOT EXISTS idx_document_revisions_content_sha256
    ON document_revisions (content_sha256);
