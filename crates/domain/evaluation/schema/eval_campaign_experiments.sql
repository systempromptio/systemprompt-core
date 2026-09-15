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
