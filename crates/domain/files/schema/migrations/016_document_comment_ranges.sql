-- Comments v2: a range anchored by two engine cursors inside the comment's
-- node, the users a comment mentions, and the edit and soft-delete stamps.

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS anchor_start TEXT;

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS anchor_end TEXT;

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS mentions TEXT[] NOT NULL DEFAULT '{}';

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS edited_at TIMESTAMPTZ;

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
