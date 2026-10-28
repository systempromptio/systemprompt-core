-- Stable node ids per revision, stored write acknowledgements keyed by the
-- caller's op id, and the request attribution of each revision.

CREATE TABLE IF NOT EXISTS document_node_ids (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    revision_id UUID NOT NULL REFERENCES document_revisions(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL CHECK (length(node_id) > 0),
    path TEXT NOT NULL,
    hash8 TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    PRIMARY KEY (document_id, revision_id, node_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_node_ids_revision_path
    ON document_node_ids (revision_id, path);
CREATE INDEX IF NOT EXISTS idx_document_node_ids_document_node
    ON document_node_ids (document_id, node_id);

CREATE TABLE IF NOT EXISTS document_write_acks (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    op_id TEXT NOT NULL CHECK (length(op_id) > 0),
    revision_id UUID NOT NULL REFERENCES document_revisions(id) ON DELETE CASCADE,
    ack JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (document_id, op_id)
);

CREATE INDEX IF NOT EXISTS idx_document_write_acks_created
    ON document_write_acks (created_at);

ALTER TABLE document_revisions
    ADD COLUMN IF NOT EXISTS session_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS context_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS trace_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS task_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS on_behalf_of VARCHAR(255);
