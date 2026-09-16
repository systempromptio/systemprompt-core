CREATE TABLE IF NOT EXISTS eval_campaign_diagnostics (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    campaign_id TEXT REFERENCES eval_campaigns(id),
    operation_key TEXT NOT NULL,
    stage TEXT NOT NULL,
    code TEXT NOT NULL,
    remediation TEXT NOT NULL,
    actor_id TEXT NOT NULL REFERENCES users(id),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    occurrences BIGINT NOT NULL DEFAULT 1,
    resolved_at TIMESTAMPTZ,
    UNIQUE(owner_id,operation_key,stage,code)
);
CREATE INDEX IF NOT EXISTS eval_campaign_diagnostics_cursor ON eval_campaign_diagnostics(owner_id,campaign_id,id);
