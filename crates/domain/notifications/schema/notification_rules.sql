CREATE TABLE IF NOT EXISTS notification_rules (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    priority INT NOT NULL,
    owner_user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    match JSONB NOT NULL,
    action JSONB NOT NULL,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_notification_rules_owner
    ON notification_rules (owner_user_id, priority);
