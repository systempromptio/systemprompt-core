-- Reopening a resolved comment: who reopened it and when. A reopen clears
-- resolved_at and resolved_by; these record the last reopen.

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS reopened_at TIMESTAMPTZ;

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS reopened_by VARCHAR(255);
