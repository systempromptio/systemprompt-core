CREATE TABLE IF NOT EXISTS analytics_fact_changes (
    owner_id TEXT NOT NULL REFERENCES users(id),
    change_id TEXT NOT NULL,
    fact_kind TEXT NOT NULL CHECK (fact_kind IN ('invocation','request','assessment','resource_association')),
    source TEXT NOT NULL CHECK (length(source) BETWEEN 1 AND 128),
    fact_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    occurred_at TIMESTAMPTZ NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    payload JSONB,
    payload_digest TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','leased','applied','superseded')),
    lease_worker TEXT,
    lease_epoch BIGINT NOT NULL DEFAULT 0,
    lease_until TIMESTAMPTZ,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error TEXT,
    applied_at TIMESTAMPTZ,
    PRIMARY KEY(owner_id,change_id),
    UNIQUE(owner_id,fact_kind,source,fact_id,revision)
);
CREATE INDEX IF NOT EXISTS analytics_fact_pending ON analytics_fact_changes(owner_id,next_attempt_at,recorded_at,change_id) WHERE state IN ('pending','leased');
CREATE TABLE IF NOT EXISTS analytics_normalized_facts (
    owner_id TEXT NOT NULL REFERENCES users(id),
    fact_kind TEXT NOT NULL,
    source TEXT NOT NULL,
    fact_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    occurred_at TIMESTAMPTZ NOT NULL,
    deleted BOOLEAN NOT NULL,
    fact JSONB,
    consumer_id TEXT,
    device_id TEXT,
    host TEXT,
    session_id TEXT,
    resource_id TEXT,
    resource_revision_id TEXT,
    invocation_source TEXT,
    invocation_id TEXT,
    request_source TEXT,
    request_id TEXT,
    succeeded BOOLEAN,
    currency TEXT,
    amount_micros BIGINT,
    input_tokens BIGINT,
    output_tokens BIGINT,
    latency_micros BIGINT,
    conversation_source TEXT,
    conversation_id TEXT,
    assessment_status TEXT,
    score_millionths BIGINT,
    generation BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id,fact_kind,source,fact_id),
    CHECK ((deleted AND fact IS NULL) OR (NOT deleted AND fact IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS analytics_facts_time ON analytics_normalized_facts(owner_id,fact_kind,occurred_at) WHERE NOT deleted;
CREATE INDEX IF NOT EXISTS analytics_facts_resource ON analytics_normalized_facts(owner_id,resource_id,occurred_at) WHERE NOT deleted;
CREATE INDEX IF NOT EXISTS analytics_facts_request ON analytics_normalized_facts(owner_id,request_source,request_id) WHERE NOT deleted;
CREATE TABLE IF NOT EXISTS analytics_fact_checkpoints (
    owner_id TEXT PRIMARY KEY REFERENCES users(id),
    generation BIGINT NOT NULL DEFAULT 0,
    applied_changes BIGINT NOT NULL DEFAULT 0,
    superseded_changes BIGINT NOT NULL DEFAULT 0,
    last_applied_at TIMESTAMPTZ,
    last_recorded_at TIMESTAMPTZ
);
CREATE TABLE IF NOT EXISTS analytics_fact_deltas (
    owner_id TEXT NOT NULL REFERENCES users(id),
    generation BIGINT NOT NULL,
    fact_kind TEXT NOT NULL,
    source TEXT NOT NULL,
    fact_id TEXT NOT NULL,
    before_fact JSONB,
    after_fact JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    consumed_at TIMESTAMPTZ,
    PRIMARY KEY(owner_id,generation)
);
CREATE INDEX IF NOT EXISTS analytics_fact_deltas_pending ON analytics_fact_deltas(owner_id,generation) WHERE consumed_at IS NULL;
CREATE TABLE IF NOT EXISTS analytics_fact_backfills (
    owner_id TEXT NOT NULL REFERENCES users(id),
    job_id TEXT NOT NULL,
    source TEXT NOT NULL,
    cursor TEXT NOT NULL DEFAULT '',
    generation BIGINT NOT NULL DEFAULT 0,
    pages BIGINT NOT NULL DEFAULT 0,
    facts BIGINT NOT NULL DEFAULT 0,
    complete BOOLEAN NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id,job_id)
);
CREATE TABLE IF NOT EXISTS analytics_fact_backfill_pages (
    owner_id TEXT NOT NULL,
    job_id TEXT NOT NULL,
    page_generation BIGINT NOT NULL,
    digest TEXT NOT NULL,
    PRIMARY KEY(owner_id,job_id,page_generation),
    FOREIGN KEY(owner_id,job_id) REFERENCES analytics_fact_backfills(owner_id,job_id)
);
CREATE TABLE IF NOT EXISTS analytics_fact_consumers (
    owner_id TEXT NOT NULL REFERENCES users(id),
    consumer TEXT NOT NULL CHECK(length(consumer) BETWEEN 1 AND 128),
    generation BIGINT NOT NULL DEFAULT 0,
    lease_worker TEXT,
    lease_epoch BIGINT NOT NULL DEFAULT 0,
    lease_until TIMESTAMPTZ,
    lease_through BIGINT,
    PRIMARY KEY(owner_id,consumer)
);
