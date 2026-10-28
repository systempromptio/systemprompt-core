CREATE TABLE IF NOT EXISTS document_grant_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event TEXT NOT NULL CHECK (event IN ('granted', 'changed', 'revoked', 'revoked_all', 'transferred', 'expired')),
    target_kind TEXT NOT NULL CHECK (target_kind IN ('document', 'folder')),
    target_id UUID NOT NULL,
    subject_kind TEXT NOT NULL,
    subject_value TEXT NOT NULL,
    old_level TEXT CHECK (old_level IN ('view', 'comment', 'edit', 'share', 'own')),
    new_level TEXT CHECK (new_level IN ('view', 'comment', 'edit', 'share', 'own')),
    valid_until TIMESTAMPTZ,
    actor_user_id VARCHAR(255),
    trace_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_grant_events_target
    ON document_grant_events (target_id, created_at);
