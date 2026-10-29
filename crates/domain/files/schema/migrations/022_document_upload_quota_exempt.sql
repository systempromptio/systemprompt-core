-- A save of an existing document (a bridge drive write, a REST whole-file
-- write) runs every content check as an upload but does not spend the
-- owner's daily upload quota.

ALTER TABLE document_uploads
    ADD COLUMN IF NOT EXISTS quota_exempt BOOLEAN NOT NULL DEFAULT false;
