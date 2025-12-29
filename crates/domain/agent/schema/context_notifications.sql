CREATE TABLE IF NOT EXISTS context_notifications (
    id SERIAL PRIMARY KEY,

    context_id TEXT NOT NULL,

    agent_id TEXT NOT NULL,

    notification_type TEXT NOT NULL CHECK (
        notification_type IN (
            'notifications/taskStatusUpdate',
            'notifications/artifactCreated',
            'notifications/messageAdded',
            'notifications/contextUpdated'
        )
    ),

    notification_data JSONB NOT NULL,

    received_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    broadcasted BOOLEAN NOT NULL DEFAULT FALSE,

    FOREIGN KEY (context_id) REFERENCES user_contexts(context_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_notifications_context
    ON context_notifications(context_id, received_at DESC);

CREATE INDEX IF NOT EXISTS idx_notifications_not_broadcasted
    ON context_notifications(broadcasted)
    WHERE broadcasted = FALSE;

CREATE INDEX IF NOT EXISTS idx_notifications_agent
    ON context_notifications(agent_id, received_at DESC);

CREATE INDEX IF NOT EXISTS idx_notifications_type
    ON context_notifications(notification_type);
