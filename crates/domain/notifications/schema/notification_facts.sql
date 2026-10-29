CREATE TABLE IF NOT EXISTS notification_facts (
    notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    outbox_id TEXT NOT NULL,
    recipient_user_id TEXT NOT NULL,
    PRIMARY KEY (recipient_user_id, outbox_id)
);

CREATE INDEX IF NOT EXISTS idx_notification_facts_notification
    ON notification_facts (notification_id);
