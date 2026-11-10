-- Blob collections in flight: garbage collection deletes a blob's row and
-- records it here in one transaction, then deletes the object and this row.
-- A collection interrupted between the two is finished by the next pass, so
-- an object whose row was collected is never left behind.

CREATE TABLE IF NOT EXISTS document_blob_collections (
    content_sha256 TEXT PRIMARY KEY CHECK (content_sha256 ~ '^[0-9a-f]{64}$'),
    storage_id TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
