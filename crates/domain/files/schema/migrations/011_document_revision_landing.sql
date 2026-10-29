CREATE TABLE IF NOT EXISTS document_node_counters (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    writer TEXT NOT NULL CHECK (length(writer) > 0),
    next BIGINT NOT NULL CHECK (next > 0),
    PRIMARY KEY (document_id, writer)
);

INSERT INTO document_node_counters (document_id, writer, next)
SELECT document_id, split_part(node_id, '.', 1), MAX(split_part(node_id, '.', 2)::bigint) + 1
FROM document_node_ids
WHERE split_part(node_id, '.', 2) ~ '^[0-9]{1,18}$'
GROUP BY document_id, split_part(node_id, '.', 1)
ORDER BY document_id, split_part(node_id, '.', 1)
ON CONFLICT (document_id, writer)
    DO UPDATE SET next = GREATEST(document_node_counters.next, EXCLUDED.next);

ALTER TABLE document_revisions ADD COLUMN IF NOT EXISTS landed_at TIMESTAMPTZ;

WITH RECURSIVE chain (id, parent_revision_id) AS (
    SELECT r.id, r.parent_revision_id
    FROM documents d
    JOIN document_revisions r ON r.id = d.head_revision_id
    UNION
    SELECT p.id, p.parent_revision_id
    FROM chain c
    JOIN document_revisions p ON p.id = c.parent_revision_id
)
UPDATE document_revisions r
SET landed_at = r.created_at
FROM chain
WHERE r.id = chain.id AND r.landed_at IS NULL;

ALTER TABLE document_write_acks ADD COLUMN IF NOT EXISTS user_id VARCHAR(255);
ALTER TABLE document_write_acks ADD COLUMN IF NOT EXISTS request_sha256 TEXT;
