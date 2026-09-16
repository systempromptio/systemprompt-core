CREATE TABLE IF NOT EXISTS eval_session_bindings (
    session_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    owner_id TEXT NOT NULL,
    fencing_token BIGINT NOT NULL,
    traffic_class TEXT NOT NULL DEFAULT 'live_evaluation' CHECK(traffic_class IN ('fixture','live_evaluation','suggestion','judge')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
DROP TRIGGER IF EXISTS eval_session_owner_scope ON eval_session_bindings;
CREATE TRIGGER eval_session_owner_scope BEFORE INSERT OR UPDATE ON eval_session_bindings FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
