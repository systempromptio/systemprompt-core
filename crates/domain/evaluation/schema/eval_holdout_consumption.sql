CREATE TABLE IF NOT EXISTS eval_holdout_consumption (
    owner_id TEXT NOT NULL REFERENCES users(id),
    case_revision_id TEXT NOT NULL REFERENCES eval_resource_revisions(id),
    experiment_id TEXT NOT NULL REFERENCES eval_experiments(id),
    consumed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(owner_id,case_revision_id)
);
DROP TRIGGER IF EXISTS eval_holdout_owner_scope ON eval_holdout_consumption;
CREATE TRIGGER eval_holdout_owner_scope BEFORE INSERT OR UPDATE ON eval_holdout_consumption FOR EACH ROW EXECUTE FUNCTION enforce_eval_lifecycle_owner();
