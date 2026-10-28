CREATE TABLE IF NOT EXISTS document_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL CHECK (length(node_id) > 0),
    anchor_find TEXT,
    anchor_nth INTEGER,
    author_user_id VARCHAR(255) NOT NULL,
    body TEXT NOT NULL CHECK (length(body) > 0),
    parent_id UUID REFERENCES document_comments(id) ON DELETE CASCADE,
    resolved_at TIMESTAMPTZ,
    resolved_by VARCHAR(255),
    answering_revision_id UUID REFERENCES document_revisions(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    anchor_start TEXT,
    anchor_end TEXT,
    mentions TEXT[] NOT NULL DEFAULT '{}',
    edited_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT document_comments_anchor_nth CHECK (anchor_nth IS NULL OR anchor_find IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_document_comments_document_created
    ON document_comments (document_id, created_at);
CREATE INDEX IF NOT EXISTS idx_document_comments_document_node
    ON document_comments (document_id, node_id);
CREATE INDEX IF NOT EXISTS idx_document_comments_author_created
    ON document_comments (author_user_id, created_at DESC);
