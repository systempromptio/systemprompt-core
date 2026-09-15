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
