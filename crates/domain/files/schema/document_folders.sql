CREATE TABLE IF NOT EXISTS document_folders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_id UUID REFERENCES document_folders(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK (length(name) > 0),
    ancestor_ids UUID[] NOT NULL DEFAULT '{}',
    owner_user_id VARCHAR(255) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ,
    agent_edit_policy TEXT NOT NULL DEFAULT 'direct' CHECK (agent_edit_policy IN ('direct', 'propose')),
    agent_cap TEXT CHECK (agent_cap IN ('none', 'read', 'comment', 'suggest', 'edit'))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_document_folders_parent_name
    ON document_folders (parent_id, name) NULLS NOT DISTINCT
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_document_folders_ancestor_ids
    ON document_folders USING gin (ancestor_ids);
CREATE INDEX IF NOT EXISTS idx_document_folders_owner
    ON document_folders (owner_user_id) WHERE deleted_at IS NULL;
