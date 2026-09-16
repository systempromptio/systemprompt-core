CREATE TABLE IF NOT EXISTS eval_campaign_holdout_proposals (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    campaign_id TEXT NOT NULL REFERENCES eval_campaigns(id),
    development_experiment_id TEXT NOT NULL REFERENCES eval_experiments(id),
    operation_key TEXT NOT NULL,
    spec JSONB NOT NULL,
    spec_digest TEXT NOT NULL,
    development_cases INTEGER NOT NULL,
    holdout_cases INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    confirmed_by TEXT REFERENCES users(id),
    confirmed_at TIMESTAMPTZ,
    experiment_id TEXT REFERENCES eval_experiments(id),
    UNIQUE(owner_id,campaign_id,operation_key)
);
