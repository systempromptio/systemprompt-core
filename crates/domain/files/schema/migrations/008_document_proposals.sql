-- Reviewable agent revisions and the per-folder agent edit policy.

CREATE TABLE IF NOT EXISTS document_proposals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    base_revision_id UUID NOT NULL REFERENCES document_revisions(id) ON DELETE CASCADE,
    proposed_revision_id UUID NOT NULL REFERENCES document_revisions(id) ON DELETE CASCADE,
    proposer_user_id VARCHAR(255) NOT NULL,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'agent', 'bridge', 'import', 'system')),
    origin_kind TEXT NOT NULL CHECK (origin_kind IN ('mcp_execution', 'bridge_device', 'web_session', 'edit_session', 'api')),
    origin_id TEXT,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'accepted', 'rejected', 'superseded')),
    message TEXT,
    reviewer_user_id VARCHAR(255),
    reviewed_at TIMESTAMPTZ,
    note TEXT,
    landed_revision_id UUID REFERENCES document_revisions(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_proposals_document_status_created
    ON document_proposals (document_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_document_proposals_proposer_open
    ON document_proposals (proposer_user_id, document_id) WHERE status = 'open';

ALTER TABLE document_folders
    ADD COLUMN IF NOT EXISTS agent_edit_policy TEXT NOT NULL DEFAULT 'direct'
        CHECK (agent_edit_policy IN ('direct', 'propose'));
