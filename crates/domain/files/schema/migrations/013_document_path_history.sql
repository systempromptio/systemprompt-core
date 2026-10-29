-- The path history the bridge Drive change feed reads: a trash records the
-- path the document left, a restore the path it returned to, and a rename or
-- move both.

CREATE TABLE IF NOT EXISTS document_path_changes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    from_path TEXT,
    to_path TEXT,
    changed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (from_path IS NOT NULL OR to_path IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_document_path_changes_changed_at
    ON document_path_changes (changed_at, id);
CREATE INDEX IF NOT EXISTS idx_document_path_changes_document
    ON document_path_changes (document_id);
