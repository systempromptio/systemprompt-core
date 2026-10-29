CREATE TABLE IF NOT EXISTS notification_receipts (
    id BIGSERIAL PRIMARY KEY,
    notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    surface TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('delivered', 'seen', 'read', 'acted', 'escalated')),
    at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_notification_receipts_state UNIQUE (notification_id, surface, state)
);
