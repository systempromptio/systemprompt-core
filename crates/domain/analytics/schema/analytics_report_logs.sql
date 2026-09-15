CREATE TABLE IF NOT EXISTS analytics_report_logs (
    id TEXT PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    level VARCHAR(50) NOT NULL,
    module VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    user_id VARCHAR(255),
    session_id VARCHAR(255),
    task_id VARCHAR(255)
);
CREATE INDEX IF NOT EXISTS idx_ar_logs_timestamp ON analytics_report_logs (timestamp);
CREATE INDEX IF NOT EXISTS idx_ar_logs_user_id ON analytics_report_logs (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_logs_session_id ON analytics_report_logs (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_logs_task_id ON analytics_report_logs (task_id);
