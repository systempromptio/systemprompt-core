-- A session opened and finished in one call (an ingest of outside bytes, an
-- inline body) is marked one-shot: it is not held to, and does not count
-- toward, the open-session cap, though its bytes count toward the quota.

ALTER TABLE document_uploads
    ADD COLUMN IF NOT EXISTS one_shot BOOLEAN NOT NULL DEFAULT false;
