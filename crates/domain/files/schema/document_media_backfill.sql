-- Documents whose image admissions have not yet been read from their
-- revisions: they hold revisions written before admissions were recorded.
-- The document_media_backfill job and the first image read or write of such
-- a document admit every stored image its revisions name, then dequeue it.
-- While any document is queued no blob row is deleted, whichever release
-- runs the garbage collection, so no image a queued document names is
-- collected before it is admitted.

CREATE TABLE IF NOT EXISTS document_media_backfill (
    document_id UUID PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    queued_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE OR REPLACE FUNCTION document_blobs_hold_during_media_backfill()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM document_media_backfill) THEN
        RETURN NULL;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER document_blobs_hold_during_media_backfill
    BEFORE DELETE ON document_blobs
    FOR EACH ROW
    EXECUTE FUNCTION document_blobs_hold_during_media_backfill();
