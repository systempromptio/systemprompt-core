CREATE TABLE IF NOT EXISTS task_push_notification_configs (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
    task_id TEXT NOT NULL,
    url TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    token TEXT,
    headers JSONB,
    authentication JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (task_id) REFERENCES agent_tasks(task_id) ON DELETE CASCADE,
    UNIQUE(task_id, endpoint)
);

CREATE INDEX IF NOT EXISTS idx_task_push_notification_configs_task_id
    ON task_push_notification_configs(task_id);

CREATE INDEX IF NOT EXISTS idx_task_push_notification_configs_created
    ON task_push_notification_configs(created_at DESC);
