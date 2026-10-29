-- Delivery sequencing and retry bookkeeping: `delivery_seq` and
-- `pending_since` advance on every transition to `pending`, the sweep grace
-- keys on `pending_since`, and delivery failures link to their notification
-- so channel retries are bounded per row. Adds the unread-age index the
-- unread retention purge reads.

ALTER TABLE notifications ADD COLUMN IF NOT EXISTS delivery_seq BIGINT NOT NULL DEFAULT 0;
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS pending_since TIMESTAMPTZ NOT NULL DEFAULT now();
ALTER TABLE notification_delivery_failures ADD COLUMN IF NOT EXISTS notification_id UUID;

DROP INDEX IF EXISTS idx_notifications_delivery_pending;
CREATE INDEX IF NOT EXISTS idx_notifications_pending_since
    ON notifications (pending_since) WHERE delivery_state IN ('pending', 'held');
CREATE INDEX IF NOT EXISTS idx_notifications_unread_age
    ON notifications (created_at) WHERE read_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_notification_delivery_failures_notification
    ON notification_delivery_failures (notification_id, stage) WHERE notification_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_notification_delivery_failures_outbox
    ON notification_delivery_failures (outbox_id, stage) WHERE outbox_id IS NOT NULL;
