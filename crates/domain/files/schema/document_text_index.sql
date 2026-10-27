CREATE TABLE IF NOT EXISTS document_text_index (
    document_id UUID PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    revision_id UUID NOT NULL REFERENCES document_revisions(id) ON DELETE CASCADE,
    plain_text TEXT NOT NULL,
    body TSVECTOR NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_text_index_body ON document_text_index USING gin (body);
