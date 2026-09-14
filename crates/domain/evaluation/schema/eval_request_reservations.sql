CREATE TABLE IF NOT EXISTS eval_request_reservations (
    request_id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    reservation_id TEXT NOT NULL UNIQUE REFERENCES eval_budget_reservations(id),
    traffic_class TEXT NOT NULL DEFAULT 'live_evaluation' CHECK(traffic_class IN ('fixture','live_evaluation','suggestion','judge')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
DROP TRIGGER IF EXISTS eval_request_owner_scope ON eval_request_reservations;
CREATE TRIGGER eval_request_owner_scope BEFORE INSERT OR UPDATE ON eval_request_reservations FOR EACH ROW EXECUTE FUNCTION enforce_eval_owner_scope();
