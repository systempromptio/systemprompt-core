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
