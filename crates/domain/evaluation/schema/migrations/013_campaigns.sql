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
CREATE TABLE IF NOT EXISTS eval_campaign_experiments (
    campaign_id TEXT NOT NULL REFERENCES eval_campaigns(id),
    owner_id TEXT NOT NULL REFERENCES users(id),
    experiment_id TEXT NOT NULL REFERENCES eval_experiments(id),
    iteration INTEGER NOT NULL CHECK(iteration>0),
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(campaign_id,iteration),
    UNIQUE(experiment_id),
    FOREIGN KEY(owner_id,campaign_id) REFERENCES eval_campaigns(owner_id,id),
    FOREIGN KEY(owner_id,experiment_id) REFERENCES eval_experiments(owner_id,id)
);
CREATE TABLE IF NOT EXISTS eval_campaign_source_changes (
    campaign_id TEXT NOT NULL REFERENCES eval_campaigns(id),
    experiment_id TEXT NOT NULL REFERENCES eval_experiments(id),
    candidate_revision_id TEXT NOT NULL,
    accepted_revision_id TEXT NOT NULL,
    change JSONB NOT NULL,
    change_digest TEXT NOT NULL,
    verified_by TEXT NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(campaign_id,change_digest)
);
CREATE TABLE IF NOT EXISTS eval_campaign_events (
    campaign_id TEXT NOT NULL REFERENCES eval_campaigns(id),
    generation BIGINT NOT NULL,
    actor_id TEXT NOT NULL REFERENCES users(id),
    action TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(campaign_id,generation)
);
CREATE INDEX IF NOT EXISTS eval_campaigns_owner_cursor ON eval_campaigns(owner_id,id);

