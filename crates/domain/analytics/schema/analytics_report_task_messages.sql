CREATE TABLE IF NOT EXISTS analytics_report_task_messages (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_task_messages_task_id ON analytics_report_task_messages (task_id);
CREATE INDEX IF NOT EXISTS idx_ar_task_messages_created_at ON analytics_report_task_messages (created_at);
