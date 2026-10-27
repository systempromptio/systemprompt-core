CREATE TABLE IF NOT EXISTS document_edit_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    base_revision_id UUID REFERENCES document_revisions(id) ON DELETE SET NULL,
    snapshot_sha256 TEXT REFERENCES document_blobs(content_sha256) ON DELETE RESTRICT,
    checkpoint_revision_id UUID REFERENCES document_revisions(id) ON DELETE SET NULL,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_edit_sessions_open
    ON document_edit_sessions (document_id) WHERE closed_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_document_edit_sessions_snapshot
    ON document_edit_sessions (snapshot_sha256) WHERE snapshot_sha256 IS NOT NULL;
