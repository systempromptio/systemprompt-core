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
    operation_key TEXT,
    operation_digest TEXT,
    CONSTRAINT eval_suggestion_operation_pair CHECK ((operation_key IS NULL AND operation_digest IS NULL) OR (operation_key IS NOT NULL AND operation_digest IS NOT NULL AND length(operation_key) BETWEEN 1 AND 200 AND operation_digest ~ '^[0-9a-f]{64}$')),
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','accepted','rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,id)
);
DROP TRIGGER IF EXISTS eval_suggestion_owner_scope ON eval_suggestions;
CREATE TRIGGER eval_suggestion_owner_scope BEFORE INSERT OR UPDATE ON eval_suggestions FOR EACH ROW EXECUTE FUNCTION enforce_eval_lifecycle_owner();

CREATE UNIQUE INDEX IF NOT EXISTS eval_suggestion_operation_unique ON eval_suggestions(owner_id,operation_key) WHERE operation_key IS NOT NULL;
