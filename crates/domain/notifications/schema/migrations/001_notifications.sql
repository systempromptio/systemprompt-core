-- Notification streams, subscriptions, rules, per-recipient rows, receipts
-- and delivery failures.

CREATE TABLE IF NOT EXISTS notification_streams (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9_-]{1,64}$'),
    name TEXT NOT NULL,
    description TEXT,
    audience TEXT[] NOT NULL,
    publishers TEXT[] NOT NULL,
    attention_default TEXT NOT NULL CHECK (attention_default IN ('silent', 'agent', 'human')),
    attention_floor TEXT CHECK (attention_floor IN ('silent', 'agent', 'human')),
    attention_ceiling TEXT CHECK (attention_ceiling IN ('silent', 'agent', 'human')),
    retention_days INT CHECK (retention_days > 0),
    coalesce_window_secs INT CHECK (coalesce_window_secs >= 0),
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

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

CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recipient_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    stream_id TEXT REFERENCES notification_streams(id) ON DELETE SET NULL,
    actor_user_id TEXT NOT NULL,
    actor_is_agent BOOLEAN NOT NULL,
    via_agent TEXT,
    subjects JSONB NOT NULL,
    primary_subject_kind TEXT,
    primary_subject_id TEXT,
    attention TEXT NOT NULL CHECK (attention IN ('silent', 'agent', 'human')),
    attention_reason TEXT NOT NULL,
    action JSONB NOT NULL,
    excerpt TEXT,
    coalesce_key TEXT NOT NULL,
    coalesce_count INT NOT NULL DEFAULT 1,
    first_event_at TIMESTAMPTZ NOT NULL,
    last_event_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    read_at TIMESTAMPTZ,
    escalated_from TEXT CHECK (escalated_from IN ('silent', 'agent', 'human')),
    escalation_note TEXT,
    held_until TIMESTAMPTZ,
    outbox_id TEXT NOT NULL,
    delivery_state TEXT NOT NULL DEFAULT 'pending'
        CHECK (delivery_state IN ('pending', 'delivered', 'held'))
);

CREATE INDEX IF NOT EXISTS idx_notifications_unread
    ON notifications (recipient_user_id, created_at DESC, id DESC) WHERE read_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_notifications_feed
    ON notifications (recipient_user_id, created_at DESC, id DESC);
CREATE UNIQUE INDEX IF NOT EXISTS uq_notifications_unread_coalesce
    ON notifications (recipient_user_id, coalesce_key) WHERE read_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_notifications_read_at
    ON notifications (read_at) WHERE read_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_notifications_held
    ON notifications (held_until) WHERE delivery_state = 'held';
CREATE INDEX IF NOT EXISTS idx_notifications_delivery_pending
    ON notifications (created_at) WHERE delivery_state IN ('pending', 'held');
CREATE INDEX IF NOT EXISTS idx_notifications_stream_created
    ON notifications (stream_id, created_at) WHERE stream_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS notification_facts (
    notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    outbox_id TEXT NOT NULL,
    recipient_user_id TEXT NOT NULL,
    PRIMARY KEY (recipient_user_id, outbox_id)
);

CREATE INDEX IF NOT EXISTS idx_notification_facts_notification
    ON notification_facts (notification_id);

CREATE TABLE IF NOT EXISTS notification_receipts (
    id BIGSERIAL PRIMARY KEY,
    notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    surface TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('delivered', 'seen', 'read', 'acted', 'escalated')),
    at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_notification_receipts_state UNIQUE (notification_id, surface, state)
);

CREATE TABLE IF NOT EXISTS notification_delivery_failures (
    id BIGSERIAL PRIMARY KEY,
    outbox_id TEXT,
    recipient_user_id TEXT,
    stage TEXT NOT NULL,
    reason TEXT NOT NULL,
    at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_notification_delivery_failures_at
    ON notification_delivery_failures (at);

