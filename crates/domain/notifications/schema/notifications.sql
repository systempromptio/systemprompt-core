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
        CHECK (delivery_state IN ('pending', 'delivered', 'held')),
    delivery_seq BIGINT NOT NULL DEFAULT 0,
    pending_since TIMESTAMPTZ NOT NULL DEFAULT now()
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
CREATE INDEX IF NOT EXISTS idx_notifications_pending_since
    ON notifications (pending_since) WHERE delivery_state IN ('pending', 'held');
CREATE INDEX IF NOT EXISTS idx_notifications_unread_age
    ON notifications (created_at) WHERE read_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_notifications_stream_created
    ON notifications (stream_id, created_at) WHERE stream_id IS NOT NULL;
