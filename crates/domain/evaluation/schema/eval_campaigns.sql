CREATE TABLE IF NOT EXISTS eval_campaigns (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    created_by TEXT NOT NULL REFERENCES users(id),
    resource_id TEXT NOT NULL,
    baseline_revision_id TEXT NOT NULL,
    budget_id TEXT NOT NULL REFERENCES eval_budget_accounts(id),
    policy JSONB NOT NULL,
    policy_digest TEXT NOT NULL,
    operation_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','paused','completed','cancelled')),
    generation BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,operation_key),
    UNIQUE(owner_id,id),
    FOREIGN KEY(owner_id,budget_id) REFERENCES eval_budget_accounts(owner_id,id)
);
