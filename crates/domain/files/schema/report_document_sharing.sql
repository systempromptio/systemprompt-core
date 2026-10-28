-- One row per grant change: who granted, changed, revoked or transferred
-- what, and the expiry sweep's removals; changes made by a since-deleted
-- user are hidden.
CREATE OR REPLACE VIEW report_document_sharing AS
SELECT
    src.id, src.event, src.target_kind, src.target_id, src.subject_kind,
    src.subject_value, src.old_level, src.new_level, src.valid_until,
    src.actor_user_id, src.trace_id, src.created_at
FROM document_grant_events src
WHERE src.actor_user_id IS NULL OR NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.actor_user_id AND d.status = 'deleted'
);
