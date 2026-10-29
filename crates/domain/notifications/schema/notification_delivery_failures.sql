CREATE TABLE IF NOT EXISTS notification_delivery_failures (
    id BIGSERIAL PRIMARY KEY,
    outbox_id TEXT,
    recipient_user_id TEXT,
    stage TEXT NOT NULL,
    reason TEXT NOT NULL,
    at TIMESTAMPTZ NOT NULL DEFAULT now(),
    notification_id UUID
);

CREATE INDEX IF NOT EXISTS idx_notification_delivery_failures_at
    ON notification_delivery_failures (at);
CREATE INDEX IF NOT EXISTS idx_notification_delivery_failures_notification
    ON notification_delivery_failures (notification_id, stage) WHERE notification_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_notification_delivery_failures_outbox
    ON notification_delivery_failures (outbox_id, stage) WHERE outbox_id IS NOT NULL;
