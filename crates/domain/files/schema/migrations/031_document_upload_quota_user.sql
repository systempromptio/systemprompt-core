-- A save of an existing document is charged to the document's owner, so the
-- daily upload quota is summed by the account charged, not by the uploader.
-- The column is backfilled in batches, proved present by a validated check
-- (so SET NOT NULL skips its own scan), and indexed CONCURRENTLY by the
-- following migration.
ALTER TABLE document_uploads ADD COLUMN IF NOT EXISTS quota_user_id VARCHAR(255);

DO $$
BEGIN
    LOOP
        UPDATE document_uploads SET quota_user_id = owner_user_id
        WHERE id IN (SELECT id FROM document_uploads WHERE quota_user_id IS NULL LIMIT 5000);
        EXIT WHEN NOT FOUND;
    END LOOP;
END
$$;

ALTER TABLE document_uploads DROP CONSTRAINT IF EXISTS document_uploads_quota_user_present;
ALTER TABLE document_uploads ADD CONSTRAINT document_uploads_quota_user_present
    CHECK (quota_user_id IS NOT NULL) NOT VALID;
ALTER TABLE document_uploads VALIDATE CONSTRAINT document_uploads_quota_user_present;
ALTER TABLE document_uploads ALTER COLUMN quota_user_id SET NOT NULL;
ALTER TABLE document_uploads DROP CONSTRAINT document_uploads_quota_user_present;
