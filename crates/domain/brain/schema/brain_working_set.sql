CREATE TABLE IF NOT EXISTS brain_working_set (
    scope_key TEXT NOT NULL,
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    importance_base FLOAT8 NOT NULL,
    PRIMARY KEY (scope_key, fact_id)
);

CREATE INDEX IF NOT EXISTS brain_working_set_scope_idx
    ON brain_working_set (scope_key, importance_base DESC, fact_id);
