CREATE TABLE IF NOT EXISTS document_grants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID REFERENCES documents(id) ON DELETE CASCADE,
    folder_id UUID REFERENCES document_folders(id) ON DELETE CASCADE,
    subject_kind TEXT NOT NULL CHECK (length(subject_kind) > 0),
    subject_value TEXT NOT NULL CHECK (length(subject_value) > 0),
    access_level TEXT NOT NULL CHECK (access_level IN ('view', 'comment', 'edit', 'share', 'own')),
    granted_by VARCHAR(255) NOT NULL,
    valid_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT document_grants_one_target CHECK ((document_id IS NULL) <> (folder_id IS NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_grants_target_subject
    ON document_grants (document_id, folder_id, subject_kind, subject_value) NULLS NOT DISTINCT;
CREATE INDEX IF NOT EXISTS idx_document_grants_subject
    ON document_grants (subject_kind, subject_value);
CREATE INDEX IF NOT EXISTS idx_document_grants_folder_id
    ON document_grants (folder_id) WHERE folder_id IS NOT NULL;
