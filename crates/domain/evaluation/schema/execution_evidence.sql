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
    traffic_class TEXT NOT NULL DEFAULT 'live_evaluation' CHECK(traffic_class IN ('fixture','live_evaluation','suggestion','judge')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS eval_request_reservations (
    request_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    reservation_id TEXT NOT NULL UNIQUE REFERENCES eval_budget_reservations(id),
    traffic_class TEXT NOT NULL DEFAULT 'live_evaluation' CHECK(traffic_class IN ('fixture','live_evaluation','suggestion','judge')),
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

CREATE OR REPLACE FUNCTION enforce_eval_owner_scope() RETURNS trigger AS $$
DECLARE expected_owner text; related_owner text;
BEGIN
    IF TG_TABLE_NAME = 'eval_executions' THEN
        SELECT owner_id INTO expected_owner FROM eval_experiments WHERE id=NEW.experiment_id;
        SELECT owner_id INTO related_owner FROM eval_resource_revisions WHERE id=NEW.case_revision_id;
    ELSIF TG_TABLE_NAME = 'eval_session_bindings' THEN
        SELECT e.owner_id INTO expected_owner FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=NEW.execution_id;
        related_owner := NEW.owner_id;
    ELSIF TG_TABLE_NAME = 'eval_execution_capabilities' THEN
        SELECT e.owner_id INTO expected_owner FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=NEW.execution_id;
        SELECT w.owner_id INTO related_owner FROM eval_workers w JOIN user_sessions s ON s.user_id=w.owner_id WHERE w.id=NEW.worker_id AND s.session_id=NEW.session_id;
    ELSIF TG_TABLE_NAME = 'eval_request_reservations' THEN
        SELECT e.owner_id INTO expected_owner FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=NEW.execution_id;
        SELECT a.owner_id INTO related_owner FROM eval_budget_reservations r JOIN eval_budget_accounts a ON a.id=r.account_id JOIN ai_requests q ON q.user_id=a.owner_id WHERE r.id=NEW.reservation_id AND q.id=NEW.request_id;
    END IF;
    IF expected_owner IS NULL OR related_owner IS NULL OR expected_owner <> related_owner THEN
        RAISE EXCEPTION 'evaluation ownership conflict' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS eval_execution_owner_scope ON eval_executions;
CREATE TRIGGER eval_execution_owner_scope BEFORE INSERT OR UPDATE OF experiment_id,case_revision_id ON eval_executions FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
DROP TRIGGER IF EXISTS eval_session_owner_scope ON eval_session_bindings;
CREATE TRIGGER eval_session_owner_scope BEFORE INSERT OR UPDATE ON eval_session_bindings FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
DROP TRIGGER IF EXISTS eval_capability_owner_scope ON eval_execution_capabilities;
CREATE TRIGGER eval_capability_owner_scope BEFORE INSERT OR UPDATE ON eval_execution_capabilities FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
DROP TRIGGER IF EXISTS eval_request_owner_scope ON eval_request_reservations;
CREATE TRIGGER eval_request_owner_scope BEFORE INSERT OR UPDATE ON eval_request_reservations FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
