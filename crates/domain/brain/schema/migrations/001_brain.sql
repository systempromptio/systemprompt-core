-- The brain store: declared plane rows, subjects and aliases, facts and
-- their revisions, the capture ledgers, recalls, outcomes and proposals.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE IF NOT EXISTS brain_fact_kinds (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9_.-]{1,64}$'),
    body JSONB NOT NULL,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS brain_topics (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9_.-]{1,64}$'),
    body JSONB NOT NULL,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS brain_interests (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9_.-]{1,64}$'),
    owner_user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    body JSONB NOT NULL,
    expires_at TIMESTAMPTZ,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_brain_interests_owner
    ON brain_interests (owner_user_id) WHERE owner_user_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS brain_workflows (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9_.-]{1,64}$'),
    body JSONB NOT NULL,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS brain_config_docs (
    id TEXT PRIMARY KEY CHECK (id IN ('filters', 'importance', 'retrieval')),
    body JSONB NOT NULL,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS brain_pipeline_versions (
    version TEXT PRIMARY KEY CHECK (version ~ '^[0-9a-f]{16}$'),
    declared_hash TEXT NOT NULL,
    code_version INT NOT NULL CHECK (code_version > 0),
    prompt_versions JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_brain_pipeline_versions_recorded
    ON brain_pipeline_versions (recorded_at DESC);

CREATE TABLE IF NOT EXISTS brain_subjects (
    subject_kind TEXT NOT NULL CHECK (length(subject_kind) BETWEEN 1 AND 64),
    subject_id TEXT NOT NULL CHECK (length(subject_id) BETWEEN 1 AND 255),
    display_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (subject_kind, subject_id)
);

CREATE TABLE IF NOT EXISTS brain_aliases (
    id TEXT PRIMARY KEY,
    subject_kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    alias TEXT NOT NULL,
    alias_norm TEXT NOT NULL CHECK (length(alias_norm) BETWEEN 1 AND 255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (subject_kind, alias_norm),
    FOREIGN KEY (subject_kind, subject_id)
        REFERENCES brain_subjects (subject_kind, subject_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_brain_aliases_subject
    ON brain_aliases (subject_kind, subject_id);
CREATE INDEX IF NOT EXISTS idx_brain_aliases_trgm
    ON brain_aliases USING GIN (alias_norm gin_trgm_ops);

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

CREATE TABLE IF NOT EXISTS brain_fact_revisions (
    id TEXT PRIMARY KEY,
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    revision INT NOT NULL CHECK (revision > 0),
    change TEXT NOT NULL CHECK (change IN (
        'created', 'revised', 'merged', 'fields_edited', 'confirmed', 'disputed',
        'dispute_resolved', 'staled', 'superseded', 'retracted', 'pinned', 'unpinned',
        'review_due', 'expired'
    )),
    actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK (status IN (
        'candidate', 'proposed', 'confirmed', 'disputed', 'stale', 'superseded', 'retired'
    )),
    statement TEXT NOT NULL,
    fields JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (fact_id, revision)
);

CREATE TABLE IF NOT EXISTS brain_extractions (
    id TEXT PRIMARY KEY,
    outbox_id TEXT NOT NULL,
    pipeline_version TEXT NOT NULL,
    extractor TEXT NOT NULL,
    kind TEXT NOT NULL,
    model TEXT,
    prompt_version TEXT,
    raw_output JSONB NOT NULL,
    input_tokens INT NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    output_tokens INT NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    candidates INT NOT NULL DEFAULT 0 CHECK (candidates >= 0),
    ai_request_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (outbox_id, pipeline_version, extractor, kind)
);

CREATE INDEX IF NOT EXISTS idx_brain_extractions_created
    ON brain_extractions (created_at);

CREATE TABLE IF NOT EXISTS brain_event_results (
    outbox_id TEXT NOT NULL,
    pipeline_version TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    outcome TEXT CHECK (outcome IN (
        'processed', 'ignored', 'no_filter', 'rejected', 'failed', 'abandoned'
    )),
    filter TEXT,
    facts_written INT NOT NULL DEFAULT 0 CHECK (facts_written >= 0),
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    PRIMARY KEY (outbox_id, pipeline_version)
);

-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_brain_event_results_claimed
    ON brain_event_results (claimed_at);

CREATE TABLE IF NOT EXISTS brain_failures (
    id TEXT PRIMARY KEY,
    outbox_id TEXT NOT NULL,
    pipeline_version TEXT,
    stage TEXT NOT NULL,
    reason TEXT NOT NULL,
    attempt INT NOT NULL DEFAULT 1 CHECK (attempt > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_brain_failures_outbox
    ON brain_failures (outbox_id);
CREATE INDEX IF NOT EXISTS idx_brain_failures_created
    ON brain_failures (created_at);

CREATE TABLE IF NOT EXISTS brain_disputes (
    id TEXT PRIMARY KEY,
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    candidate JSONB NOT NULL,
    similarity DOUBLE PRECISION,
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
    resolution TEXT,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    resolved_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_brain_disputes_fact
    ON brain_disputes (fact_id);
CREATE INDEX IF NOT EXISTS idx_brain_disputes_open
    ON brain_disputes (opened_at) WHERE status = 'open';

CREATE TABLE IF NOT EXISTS brain_tombstones (
    kind TEXT NOT NULL,
    identity_hash TEXT NOT NULL,
    erased_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    erased_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (kind, identity_hash)
);

CREATE TABLE IF NOT EXISTS brain_recalls (
    id TEXT PRIMARY KEY,
    principal_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ai_request_id TEXT,
    gateway_conversation_id TEXT,
    session_id TEXT,
    context_id TEXT,
    client TEXT NOT NULL,
    project TEXT,
    workflow TEXT,
    injected JSONB NOT NULL,
    withheld JSONB NOT NULL,
    tokens INT NOT NULL DEFAULT 0 CHECK (tokens >= 0),
    as_of TIMESTAMPTZ NOT NULL,
    skipped TEXT,
    rendered_block TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_brain_recalls_conversation
    ON brain_recalls (gateway_conversation_id, created_at DESC)
    WHERE gateway_conversation_id IS NOT NULL;
-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_brain_recalls_principal
    ON brain_recalls (principal_user_id);
-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_brain_recalls_ai_request
    ON brain_recalls (ai_request_id) WHERE ai_request_id IS NOT NULL;
-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_brain_recalls_created
    ON brain_recalls (created_at);

CREATE TABLE IF NOT EXISTS brain_outcomes (
    id TEXT PRIMARY KEY,
    recall_id TEXT NOT NULL REFERENCES brain_recalls(id) ON DELETE CASCADE,
    fact_id TEXT NOT NULL,
    signal TEXT NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (recall_id, fact_id, signal)
);

-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_brain_outcomes_fact
    ON brain_outcomes (fact_id);
-- Why: the table is created empty by this migration, so the index builds instantly.
CREATE INDEX IF NOT EXISTS idx_brain_outcomes_created
    ON brain_outcomes (created_at);

CREATE TABLE IF NOT EXISTS brain_proposals (
    id TEXT PRIMARY KEY,
    analysis TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('config_diff', 'fact_actions')),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'approved', 'rejected')),
    summary TEXT NOT NULL,
    body JSONB NOT NULL,
    replay JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    decided_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_brain_proposals_open
    ON brain_proposals (created_at DESC) WHERE status = 'open';
