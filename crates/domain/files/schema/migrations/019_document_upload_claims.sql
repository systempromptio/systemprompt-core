-- A completion claims its upload (`completing`) so an abort or the expiry
-- job cannot close it mid-assembly, and a completed upload records which
-- content check it passed (`checked_kind`), so an import can refuse an
-- editable format whose check the bytes never went through.

ALTER TABLE document_uploads DROP CONSTRAINT IF EXISTS document_uploads_status_check;

ALTER TABLE document_uploads
    ADD CONSTRAINT document_uploads_status_check
    CHECK (status IN ('open', 'completing', 'completed', 'refused', 'aborted', 'expired'));

ALTER TABLE document_uploads
    ADD COLUMN IF NOT EXISTS checked_kind TEXT;

ALTER TABLE document_uploads
    ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ;
