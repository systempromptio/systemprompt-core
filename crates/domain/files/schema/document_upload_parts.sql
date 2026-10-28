CREATE TABLE IF NOT EXISTS document_upload_parts (
    upload_id UUID NOT NULL REFERENCES document_uploads(id) ON DELETE CASCADE,
    part_number INTEGER NOT NULL CHECK (part_number >= 1),
    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0),
    sha256 TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    storage_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (upload_id, part_number)
);
