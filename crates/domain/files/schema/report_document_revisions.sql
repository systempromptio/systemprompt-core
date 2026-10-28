-- One row per document revision with its author, origin and request
-- attribution; hidden once the author's user is deleted. The view lives with
-- the files schema because analytics installs before the document tables.
CREATE OR REPLACE VIEW report_document_revisions AS
SELECT
    src.id, src.document_id, src.author_user_id, src.actor_kind,
    src.origin_kind, src.origin_id, src.session_id, src.context_id,
    src.trace_id, src.task_id, src.on_behalf_of, src.message,
    b.size_bytes, src.created_at
FROM document_revisions src
JOIN document_blobs b ON b.content_sha256 = src.content_sha256
WHERE NOT EXISTS (
    SELECT 1 FROM users d WHERE d.id = src.author_user_id AND d.status = 'deleted'
);
