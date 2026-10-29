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
