-- An upload object is recorded before its bytes are stored and forgotten
-- once a part row holds it or it is deleted, so an object left behind by an
-- interrupted part or a failed delete is found and removed by the upload
-- expiry job. Stale completion claims are found through their own index.

CREATE TABLE IF NOT EXISTS document_upload_objects (
    storage_id TEXT PRIMARY KEY,
    upload_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_upload_objects_created
    ON document_upload_objects (created_at);

CREATE INDEX IF NOT EXISTS idx_document_uploads_completing_claim
    ON document_uploads (claimed_at) WHERE status = 'completing';
