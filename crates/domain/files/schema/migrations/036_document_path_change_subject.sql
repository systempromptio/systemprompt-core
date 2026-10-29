-- A path change may name one grant subject and how that subject's access
-- changed: `subject_access = 'lost'` says the subject may have lost access to
-- the document at that landing position, and the Drive feed serves it as
-- `deleted` to a user that subject covers once the document is no longer
-- visible to them; `'gained'` says the subject gained access it did not hold,
-- and the feed serves it as `created` to a user that subject covers while the
-- document is visible to them. Rows without a subject are the document's own
-- path history, as before.
--
-- `document_drive_path` is the path a document has in the Drive tree: its
-- folder chain's names joined by `/`, then its name.
ALTER TABLE document_path_changes ADD COLUMN IF NOT EXISTS subject_kind TEXT;
ALTER TABLE document_path_changes ADD COLUMN IF NOT EXISTS subject_value TEXT;
ALTER TABLE document_path_changes ADD COLUMN IF NOT EXISTS subject_access TEXT;

ALTER TABLE document_path_changes
    DROP CONSTRAINT IF EXISTS document_path_changes_subject_pair;
ALTER TABLE document_path_changes
    ADD CONSTRAINT document_path_changes_subject_pair
    CHECK ((subject_kind IS NULL) = (subject_value IS NULL)
           AND (subject_kind IS NULL) = (subject_access IS NULL)
           AND (subject_access IS NULL OR subject_access IN ('lost', 'gained'))) NOT VALID;
ALTER TABLE document_path_changes
    VALIDATE CONSTRAINT document_path_changes_subject_pair;

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
