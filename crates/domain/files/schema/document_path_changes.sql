CREATE SEQUENCE IF NOT EXISTS document_landing_seq AS BIGINT;

CREATE TABLE IF NOT EXISTS document_path_changes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    from_path TEXT,
    to_path TEXT,
    changed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    landing_xid BIGINT DEFAULT pg_current_xact_id()::text::bigint,
    landing_seq BIGINT DEFAULT nextval('document_landing_seq'),
    subject_kind TEXT,
    subject_value TEXT,
    subject_access TEXT,
    CHECK (from_path IS NOT NULL OR to_path IS NOT NULL),
    CONSTRAINT document_path_changes_subject_pair
        CHECK ((subject_kind IS NULL) = (subject_value IS NULL)
               AND (subject_kind IS NULL) = (subject_access IS NULL)
               AND (subject_access IS NULL OR subject_access IN ('lost', 'gained')))
);

CREATE INDEX IF NOT EXISTS idx_document_path_changes_changed_at
    ON document_path_changes (changed_at, id);
CREATE INDEX IF NOT EXISTS idx_document_path_changes_document
    ON document_path_changes (document_id);
CREATE INDEX IF NOT EXISTS idx_document_path_changes_landing
    ON document_path_changes (landing_xid, landing_seq)
    WHERE landing_xid IS NOT NULL;

CREATE OR REPLACE FUNCTION document_drive_path(folder UUID, name TEXT)
RETURNS TEXT
LANGUAGE sql STABLE
AS $$
    SELECT COALESCE(
        (SELECT string_agg(a.name, '/' ORDER BY u.ord)
         FROM document_folders f
         CROSS JOIN LATERAL unnest(f.ancestor_ids || f.id) WITH ORDINALITY AS u(folder_id, ord)
         JOIN document_folders a ON a.id = u.folder_id
         WHERE f.id = folder) || '/',
        '') || name
$$;
