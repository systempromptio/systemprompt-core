-- Live co-editing: each edit session numbers its own updates in commit
-- order, and records the version vector its stored snapshot covers.

ALTER TABLE document_edit_sessions
    ADD COLUMN IF NOT EXISTS next_seq BIGINT NOT NULL DEFAULT 1;

ALTER TABLE document_edit_sessions
    ADD COLUMN IF NOT EXISTS snapshot_vv BYTEA;

UPDATE document_edit_sessions s
SET next_seq = u.max_seq + 1
FROM (
    SELECT session_id, MAX(seq) AS max_seq
    FROM document_collab_updates
    GROUP BY session_id
) u
WHERE u.session_id = s.id AND s.next_seq <= u.max_seq;
