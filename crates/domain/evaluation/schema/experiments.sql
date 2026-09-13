CREATE TABLE IF NOT EXISTS eval_resource_revisions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    resource_kind TEXT NOT NULL CHECK(resource_kind IN ('case','dataset','rubric','policy')),
    resource_key TEXT NOT NULL,
    digest TEXT NOT NULL,
    content JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,resource_kind,resource_key,digest),
    UNIQUE(owner_id,id)
);
CREATE TABLE IF NOT EXISTS eval_budget_accounts (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    cap BIGINT NOT NULL CHECK(cap>0),
    reserved BIGINT NOT NULL DEFAULT 0 CHECK(reserved>=0),
    settled BIGINT NOT NULL DEFAULT 0 CHECK(settled>=0),
    frozen BOOLEAN NOT NULL DEFAULT FALSE,
    operation_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,operation_key),
    UNIQUE(owner_id,id)
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
    owner_id TEXT NOT NULL REFERENCES users(id),
    spec JSONB NOT NULL,
    spec_digest TEXT NOT NULL,
    budget_id TEXT NOT NULL REFERENCES eval_budget_accounts(id),
    idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','running','completed','cancelled','blocked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,idempotency_key),
    UNIQUE(owner_id,id),
    FOREIGN KEY(owner_id,budget_id) REFERENCES eval_budget_accounts(owner_id,id)
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
    deadline_at TIMESTAMPTZ,
    active_runtime_ms BIGINT NOT NULL DEFAULT 0 CHECK(active_runtime_ms BETWEEN 0 AND 1800000),
    last_heartbeat_at TIMESTAMPTZ,
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
CREATE TABLE IF NOT EXISTS eval_execution_approvals (
    id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    owner_id TEXT NOT NULL REFERENCES users(id),
    fencing_token BIGINT NOT NULL,
    operation JSONB NOT NULL,
    operation_digest TEXT NOT NULL CHECK(operation_digest ~ '^[0-9a-f]{64}$'),
    precondition_digest TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','approved','denied','expired','consumed')),
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '24 hours',
    decided_by TEXT,
    decided_at TIMESTAMPTZ,
    UNIQUE(execution_id,operation_digest,precondition_digest)
);
CREATE TABLE IF NOT EXISTS eval_execution_cleanup (
    execution_id TEXT PRIMARY KEY REFERENCES eval_executions(id),
    container_id TEXT,
    network_id TEXT,
    status TEXT NOT NULL CHECK(status IN ('pending','verified','failed','retrying')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0),
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS eval_approved_operation_receipts (
    approval_id TEXT PRIMARY KEY REFERENCES eval_execution_approvals(id),
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    operation_digest TEXT NOT NULL CHECK(operation_digest ~ '^[0-9a-f]{64}$'),
    output JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE OR REPLACE FUNCTION reject_eval_operation_receipt_change() RETURNS trigger AS $$
BEGIN RAISE EXCEPTION 'approved operation receipts are immutable' USING ERRCODE='23514'; END $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS eval_operation_receipts_immutable ON eval_approved_operation_receipts;
CREATE TRIGGER eval_operation_receipts_immutable BEFORE UPDATE OR DELETE ON eval_approved_operation_receipts FOR EACH ROW EXECUTE FUNCTION reject_eval_operation_receipt_change();
CREATE TABLE IF NOT EXISTS eval_execution_measurements (
    execution_id TEXT PRIMARY KEY REFERENCES eval_executions(id),
    hard_failures TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    deterministic_checks JSONB NOT NULL DEFAULT '{}'::JSONB,
    judgment JSONB,
    quality_milli INTEGER CHECK(quality_milli BETWEEN 0 AND 5000),
    latency_ms BIGINT CHECK(latency_ms>=0),
    input_tokens BIGINT CHECK(input_tokens>=0),
    output_tokens BIGINT CHECK(output_tokens>=0),
    tool_calls BIGINT CHECK(tool_calls>=0),
    attempted_cost_microdollars BIGINT NOT NULL DEFAULT 0 CHECK(attempted_cost_microdollars>=0),
    accounting_status TEXT NOT NULL CHECK(accounting_status IN ('complete','partial','unknown')),
    verified_success BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS eval_suggestions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    experiment_id TEXT NOT NULL REFERENCES eval_experiments(id),
    candidate_revision_id TEXT,
    supporting_execution_ids TEXT[] NOT NULL,
    proposed_changes JSONB NOT NULL,
    hypothesis TEXT NOT NULL CHECK(length(hypothesis) BETWEEN 1 AND 4000),
    reservation_id TEXT NOT NULL REFERENCES eval_budget_reservations(id),
    originating_evidence JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','accepted','rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,id)
);
CREATE TABLE IF NOT EXISTS eval_holdout_consumption (
    owner_id TEXT NOT NULL REFERENCES users(id),
    case_revision_id TEXT NOT NULL REFERENCES eval_resource_revisions(id),
    experiment_id TEXT NOT NULL REFERENCES eval_experiments(id),
    consumed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(owner_id,case_revision_id)
);
CREATE TABLE IF NOT EXISTS eval_fixture_payloads (
    owner_id TEXT NOT NULL REFERENCES users(id),
    fixture_key TEXT NOT NULL,
    digest TEXT NOT NULL,
    payload JSONB NOT NULL,
    evidence_label TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(owner_id,fixture_key,digest)
);
CREATE TABLE IF NOT EXISTS eval_fixture_test_records (
    owner_id TEXT NOT NULL REFERENCES users(id),
    record_key TEXT NOT NULL,
    value JSONB NOT NULL,
    original_value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(owner_id,record_key)
);

CREATE OR REPLACE FUNCTION enforce_eval_lifecycle_owner() RETURNS trigger AS $$
DECLARE experiment_owner text; related_owner text;
BEGIN
    IF TG_TABLE_NAME = 'eval_execution_approvals' THEN
        SELECT e.owner_id INTO experiment_owner FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=NEW.execution_id;
        related_owner := NEW.owner_id;
    ELSIF TG_TABLE_NAME = 'eval_suggestions' THEN
        SELECT owner_id INTO experiment_owner FROM eval_experiments WHERE id=NEW.experiment_id;
        SELECT a.owner_id INTO related_owner FROM eval_budget_reservations r JOIN eval_budget_accounts a ON a.id=r.account_id WHERE r.id=NEW.reservation_id AND a.owner_id=NEW.owner_id;
    ELSE
        SELECT e.owner_id INTO experiment_owner FROM eval_experiments e JOIN eval_resource_revisions c ON c.id=NEW.case_revision_id AND c.owner_id=e.owner_id WHERE e.id=NEW.experiment_id;
        related_owner := NEW.owner_id;
    END IF;
    IF experiment_owner IS NULL OR related_owner IS NULL OR experiment_owner <> related_owner THEN RAISE EXCEPTION 'evaluation lifecycle ownership conflict' USING ERRCODE='23514'; END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS eval_approval_owner_scope ON eval_execution_approvals;
CREATE TRIGGER eval_approval_owner_scope BEFORE INSERT OR UPDATE ON eval_execution_approvals FOR EACH ROW EXECUTE FUNCTION enforce_eval_lifecycle_owner();
DROP TRIGGER IF EXISTS eval_suggestion_owner_scope ON eval_suggestions;
CREATE TRIGGER eval_suggestion_owner_scope BEFORE INSERT OR UPDATE ON eval_suggestions FOR EACH ROW EXECUTE FUNCTION enforce_eval_lifecycle_owner();
DROP TRIGGER IF EXISTS eval_holdout_owner_scope ON eval_holdout_consumption;
CREATE TRIGGER eval_holdout_owner_scope BEFORE INSERT OR UPDATE ON eval_holdout_consumption FOR EACH ROW EXECUTE FUNCTION enforce_eval_lifecycle_owner();
