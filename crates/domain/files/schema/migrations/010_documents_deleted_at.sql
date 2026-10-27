-- The document trash timestamp is stored in documents.deleted_at, the column
-- the declarative schema and migration 003 create.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns
               WHERE table_name = 'documents' AND column_name = 'trashed_at') THEN
        ALTER TABLE documents RENAME COLUMN trashed_at TO deleted_at;
    END IF;
END $$;

DROP INDEX IF EXISTS idx_documents_trashed_at;

CREATE INDEX IF NOT EXISTS idx_documents_deleted_at
    ON documents (deleted_at) WHERE deleted_at IS NOT NULL;
