CREATE TABLE IF NOT EXISTS eval_frozen_workspaces (
    owner_id TEXT NOT NULL,
    digest TEXT NOT NULL,
    content JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (owner_id, digest)
);
CREATE TABLE IF NOT EXISTS eval_execution_evidence (
    execution_id TEXT PRIMARY KEY REFERENCES eval_executions(id),
    fencing_token BIGINT NOT NULL,
    digest TEXT NOT NULL,
    manifest JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS eval_session_bindings (
    session_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    owner_id TEXT NOT NULL,
    fencing_token BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS eval_request_reservations (
    request_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    reservation_id TEXT NOT NULL UNIQUE REFERENCES eval_budget_reservations(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS eval_workers (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    environment TEXT NOT NULL,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '7 days',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS eval_execution_artifacts (
    execution_id TEXT PRIMARY KEY REFERENCES eval_execution_evidence(execution_id),
    content JSONB NOT NULL
);
CREATE TABLE IF NOT EXISTS eval_execution_capabilities (
    token_hash TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    worker_id TEXT NOT NULL REFERENCES eval_workers(id),
    session_id TEXT NOT NULL REFERENCES user_sessions(session_id),
    fencing_token BIGINT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS eval_capabilities_execution ON eval_execution_capabilities(execution_id);
