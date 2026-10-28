-- Resumable uploads: one row per upload session, one per stored part.
-- A completed upload names the blob it produced; the row is the audit
-- record of who uploaded what, under which trace, and why it was refused.

CREATE TABLE IF NOT EXISTS document_uploads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id VARCHAR(255) NOT NULL,
    file_name TEXT NOT NULL CHECK (length(file_name) > 0),
    declared_mime TEXT NOT NULL,
    declared_size BIGINT NOT NULL CHECK (declared_size > 0),
    declared_sha256 TEXT NOT NULL CHECK (declared_sha256 ~ '^[0-9a-f]{64}$'),
    part_size BIGINT NOT NULL CHECK (part_size > 0),
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'completed', 'refused', 'aborted', 'expired')),
    blob_sha256 TEXT CHECK (blob_sha256 ~ '^[0-9a-f]{64}$'),
    detected_mime TEXT,
    refusal TEXT,
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_document_uploads_owner_created
    ON document_uploads (owner_user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_document_uploads_open_expiry
    ON document_uploads (expires_at) WHERE status = 'open';
CREATE INDEX IF NOT EXISTS idx_document_uploads_owner_blob
    ON document_uploads (owner_user_id, blob_sha256) WHERE status = 'completed';

CREATE TABLE IF NOT EXISTS document_upload_parts (
    upload_id UUID NOT NULL REFERENCES document_uploads(id) ON DELETE CASCADE,
    part_number INTEGER NOT NULL CHECK (part_number >= 1),
    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0),
    sha256 TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    storage_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (upload_id, part_number)
);
