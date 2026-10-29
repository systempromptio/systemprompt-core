CREATE TABLE IF NOT EXISTS document_upload_objects (
    storage_id TEXT PRIMARY KEY,
    upload_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_upload_objects_created
    ON document_upload_objects (created_at);
