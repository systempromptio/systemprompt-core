-- The uploaded images each document may serve: a blob is served for a
-- document only when it was admitted to that document. A row also keeps the
-- blob from garbage collection while the document exists.

CREATE TABLE IF NOT EXISTS document_media (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    content_sha256 TEXT NOT NULL
        REFERENCES document_blobs(content_sha256) ON DELETE RESTRICT,
    admitted_by VARCHAR(255) NOT NULL,
    admitted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (document_id, content_sha256)
);

CREATE INDEX IF NOT EXISTS idx_document_media_content
    ON document_media (content_sha256);

-- Blob collections in flight: garbage collection deletes a blob's row and
-- records it here in one transaction, then deletes the object and this row.
-- A collection interrupted between the two is finished by the next pass, so
-- an object whose row was collected is never left behind.

CREATE TABLE IF NOT EXISTS document_blob_collections (
    content_sha256 TEXT PRIMARY KEY CHECK (content_sha256 ~ '^[0-9a-f]{64}$'),
    storage_id TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
