CREATE TABLE IF NOT EXISTS brain_facts (
    id TEXT PRIMARY KEY CHECK (id LIKE 'm\_%'),
    kind TEXT NOT NULL,
    subject_kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    scope_level TEXT NOT NULL CHECK (scope_level IN ('user', 'project', 'group', 'org')),
    scope_key TEXT,
    statement TEXT NOT NULL,
    fields JSONB NOT NULL DEFAULT '{}'::jsonb,
    source JSONB NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind IN (
        'document', 'comment', 'session', 'entered_by', 'external', 'run', 'aggregate'
    )),
    source_document_id TEXT,
    source_revision_id TEXT,
    source_comment_id TEXT,
    topics TEXT[] NOT NULL DEFAULT '{}',
    status TEXT NOT NULL CHECK (status IN (
        'candidate', 'proposed', 'confirmed', 'disputed', 'stale', 'superseded', 'retired'
    )),
    confirmed_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    confirmed_at TIMESTAMPTZ,
    importance_base DOUBLE PRECISION NOT NULL DEFAULT 0,
    valid_from TIMESTAMPTZ,
    valid_to TIMESTAMPTZ,
    review_after TIMESTAMPTZ,
    review_notified_at TIMESTAMPTZ,
    retention_until TIMESTAMPTZ,
    legal_hold BOOLEAN NOT NULL DEFAULT false,
    identity_hash TEXT NOT NULL,
    statement_hash TEXT NOT NULL,
    revision INT NOT NULL DEFAULT 1 CHECK (revision > 0),
    pinned BOOLEAN NOT NULL DEFAULT false,
    owner_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    pipeline_version TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    search TSVECTOR GENERATED ALWAYS AS (to_tsvector('simple', statement)) STORED,
    UNIQUE (kind, identity_hash),
    CONSTRAINT brain_facts_scope_key CHECK ((scope_level = 'org') = (scope_key IS NULL))
);

CREATE INDEX IF NOT EXISTS idx_brain_facts_subject
    ON brain_facts (subject_kind, subject_id);
CREATE INDEX IF NOT EXISTS idx_brain_facts_source_document
    ON brain_facts (source_document_id) WHERE source_document_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_brain_facts_source_comment
    ON brain_facts (source_comment_id) WHERE source_comment_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_brain_facts_status_review
    ON brain_facts (status, review_after);
CREATE INDEX IF NOT EXISTS idx_brain_facts_topics
    ON brain_facts USING GIN (topics);
CREATE INDEX IF NOT EXISTS idx_brain_facts_scope
    ON brain_facts (scope_level, scope_key);
CREATE INDEX IF NOT EXISTS idx_brain_facts_pinned
    ON brain_facts (pinned) WHERE pinned;
CREATE INDEX IF NOT EXISTS idx_brain_facts_owner
    ON brain_facts (owner_user_id) WHERE owner_user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_brain_facts_statement_hash
    ON brain_facts (kind, subject_kind, subject_id, statement_hash);
CREATE INDEX IF NOT EXISTS idx_brain_facts_search
    ON brain_facts USING GIN (search);
CREATE INDEX IF NOT EXISTS idx_brain_facts_retention
    ON brain_facts (retention_until) WHERE retention_until IS NOT NULL;
