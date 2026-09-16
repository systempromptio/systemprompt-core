CREATE TABLE IF NOT EXISTS eval_experiments (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    spec JSONB NOT NULL,
    spec_digest TEXT NOT NULL,
    budget_id TEXT NOT NULL REFERENCES eval_budget_accounts(id),
    idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','running','completed','cancelled','blocked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,idempotency_key),
    UNIQUE(owner_id,id),
    FOREIGN KEY(owner_id,budget_id) REFERENCES eval_budget_accounts(owner_id,id)
);
CREATE INDEX IF NOT EXISTS eval_experiments_owner_created ON eval_experiments(owner_id,created_at DESC);
