-- The grant records the subject's display name at the time it is written.

ALTER TABLE document_grants ADD COLUMN IF NOT EXISTS subject_display TEXT;
