CREATE TABLE IF NOT EXISTS notification_subscriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject TEXT NOT NULL,
    subject_user_id TEXT GENERATED ALWAYS AS (
        CASE WHEN subject LIKE 'user:%' THEN substr(subject, 6) END
    ) STORED,
    target_stream TEXT REFERENCES notification_streams(id) ON DELETE CASCADE,
    target_kind TEXT,
    target_id TEXT,
    attention_ceiling TEXT CHECK (attention_ceiling IN ('silent', 'agent', 'human')),
    quiet_hours JSONB,
    digest JSONB,
    source TEXT NOT NULL,
    created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT notification_subscriptions_one_target CHECK (
        (target_stream IS NOT NULL) <> (target_kind IS NOT NULL AND target_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_notification_subscriptions_stream
    ON notification_subscriptions (subject, target_stream)
    WHERE target_stream IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_notification_subscriptions_resource
    ON notification_subscriptions (subject, target_kind, target_id)
    WHERE target_kind IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_notification_subscriptions_resource
    ON notification_subscriptions (target_kind, target_id)
    WHERE target_kind IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_notification_subscriptions_stream
    ON notification_subscriptions (target_stream)
    WHERE target_stream IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_notification_subscriptions_user
    ON notification_subscriptions (subject_user_id)
    WHERE subject_user_id IS NOT NULL;
