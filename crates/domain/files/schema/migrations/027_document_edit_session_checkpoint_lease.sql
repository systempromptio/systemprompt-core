-- Live co-editing: a checkpoint of an edit session is leased so one
-- replica at a time settles it, and open sessions are found by staleness.

ALTER TABLE document_edit_sessions
    ADD COLUMN IF NOT EXISTS checkpoint_lease_id UUID;

ALTER TABLE document_edit_sessions
    ADD COLUMN IF NOT EXISTS checkpoint_lease_until TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_document_edit_sessions_open_updated
    ON document_edit_sessions (updated_at) WHERE closed_at IS NULL;
