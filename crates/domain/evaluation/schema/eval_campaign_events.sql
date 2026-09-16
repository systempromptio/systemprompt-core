CREATE TABLE IF NOT EXISTS eval_campaign_events (
    campaign_id TEXT NOT NULL REFERENCES eval_campaigns(id),
    generation BIGINT NOT NULL,
    actor_id TEXT NOT NULL REFERENCES users(id),
    action TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(campaign_id,generation)
);
CREATE INDEX IF NOT EXISTS eval_campaigns_owner_cursor ON eval_campaigns(owner_id,id);
