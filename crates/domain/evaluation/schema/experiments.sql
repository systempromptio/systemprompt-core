CREATE TABLE IF NOT EXISTS eval_resource_revisions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    resource_kind TEXT NOT NULL CHECK(resource_kind IN ('case','dataset','rubric','policy')),
    resource_key TEXT NOT NULL,
    digest TEXT NOT NULL,
    content JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,resource_kind,resource_key,digest)
);
CREATE TABLE IF NOT EXISTS eval_budget_accounts (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    cap BIGINT NOT NULL CHECK(cap>0),
    reserved BIGINT NOT NULL DEFAULT 0 CHECK(reserved>=0),
    settled BIGINT NOT NULL DEFAULT 0 CHECK(settled>=0),
    frozen BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS eval_budget_reservations (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES eval_budget_accounts(id),
    operation_key TEXT NOT NULL,
    reserved BIGINT NOT NULL CHECK(reserved>0),
    actual BIGINT CHECK(actual>=0),
    request_id TEXT UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    settled_at TIMESTAMPTZ,
    UNIQUE(account_id,operation_key)
);
CREATE TABLE IF NOT EXISTS eval_experiments (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    spec JSONB NOT NULL,
    spec_digest TEXT NOT NULL,
    budget_id TEXT NOT NULL REFERENCES eval_budget_accounts(id),
    idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','running','completed','cancelled','blocked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,idempotency_key)
);
CREATE INDEX IF NOT EXISTS eval_experiments_owner_created ON eval_experiments(owner_id,created_at DESC);
CREATE TABLE IF NOT EXISTS eval_executions (
    id TEXT PRIMARY KEY,
    experiment_id TEXT NOT NULL REFERENCES eval_experiments(id),
    variant_index INTEGER NOT NULL CHECK(variant_index>=0),
    case_revision_id TEXT NOT NULL REFERENCES eval_resource_revisions(id),
    repetition INTEGER NOT NULL CHECK(repetition>=0),
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','running','awaiting_approval','completed','error','cancelled','blocked','budget_exhausted')),
    lease_owner TEXT,
    lease_expires_at TIMESTAMPTZ,
    fencing_token BIGINT NOT NULL DEFAULT 0,
    result JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    UNIQUE(experiment_id,variant_index,case_revision_id,repetition)
);
CREATE INDEX IF NOT EXISTS eval_executions_queue ON eval_executions(status,created_at);
CREATE TABLE IF NOT EXISTS eval_execution_events (
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    sequence BIGINT NOT NULL CHECK(sequence>=0),
    payload JSONB NOT NULL,
    digest TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(execution_id,sequence)
);
