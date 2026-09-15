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
