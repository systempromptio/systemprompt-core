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
DROP TRIGGER IF EXISTS eval_capability_owner_scope ON eval_execution_capabilities;
CREATE TRIGGER eval_capability_owner_scope BEFORE INSERT OR UPDATE ON eval_execution_capabilities FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
