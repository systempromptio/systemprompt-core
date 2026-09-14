ALTER TABLE eval_executions ADD COLUMN IF NOT EXISTS active_runtime_ms BIGINT NOT NULL DEFAULT 0;
ALTER TABLE eval_executions ADD COLUMN IF NOT EXISTS last_heartbeat_at TIMESTAMPTZ;
ALTER TABLE eval_session_bindings ADD COLUMN IF NOT EXISTS traffic_class TEXT NOT NULL DEFAULT 'live_evaluation';
ALTER TABLE eval_request_reservations ADD COLUMN IF NOT EXISTS traffic_class TEXT NOT NULL DEFAULT 'live_evaluation';
DO $$ BEGIN
    ALTER TABLE eval_executions ADD CONSTRAINT eval_executions_active_runtime_bound CHECK(active_runtime_ms BETWEEN 0 AND 1800000);
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS eval_execution_approvals (
    id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    owner_id TEXT NOT NULL REFERENCES users(id),
    fencing_token BIGINT NOT NULL,
    operation JSONB NOT NULL,
    operation_digest TEXT,
    precondition_digest TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','approved','denied','expired','consumed')),
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW()+INTERVAL '24 hours',
    decided_by TEXT REFERENCES users(id),
    decided_at TIMESTAMPTZ,
    UNIQUE(execution_id,precondition_digest)
);
ALTER TABLE eval_execution_approvals ADD COLUMN IF NOT EXISTS operation_digest TEXT;
UPDATE eval_execution_approvals SET operation_digest=encode(digest(convert_to(operation::TEXT,'UTF8'),'sha256'),'hex') WHERE operation_digest IS NULL;
ALTER TABLE eval_execution_approvals ALTER COLUMN operation_digest SET NOT NULL;
DO $$ BEGIN
    ALTER TABLE eval_execution_approvals ADD CONSTRAINT eval_execution_approvals_operation_digest CHECK(operation_digest ~ '^[0-9a-f]{64}$');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
ALTER TABLE eval_execution_approvals DROP CONSTRAINT IF EXISTS eval_execution_approvals_execution_id_precondition_digest_key;
CREATE UNIQUE INDEX IF NOT EXISTS eval_execution_approvals_operation_precondition ON eval_execution_approvals(execution_id,operation_digest,precondition_digest);
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
DO $$ BEGIN
    ALTER TABLE eval_session_bindings ADD CONSTRAINT eval_session_bindings_traffic_class CHECK(traffic_class IN ('fixture','live_evaluation','suggestion','judge'));
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
DO $$ BEGIN
    ALTER TABLE eval_request_reservations ADD CONSTRAINT eval_request_reservations_traffic_class CHECK(traffic_class IN ('fixture','live_evaluation','suggestion','judge'));
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
