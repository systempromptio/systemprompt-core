CREATE TABLE IF NOT EXISTS documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    folder_id UUID REFERENCES document_folders(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(name) > 0),
    format TEXT NOT NULL CHECK (format IN ('docx', 'xlsx', 'pptx', 'odt', 'ods', 'odp', 'html', 'markdown', 'text')),
    mime_type TEXT NOT NULL,
    head_revision_id UUID,
    template_id TEXT,
    owner_user_id VARCHAR(255) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_documents_folder_name
    ON documents (folder_id, name) NULLS NOT DISTINCT
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_documents_updated
    ON documents (updated_at DESC, id DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_documents_owner_updated
    ON documents (owner_user_id, updated_at DESC, id DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_documents_folder_id
    ON documents (folder_id) WHERE deleted_at IS NULL;
