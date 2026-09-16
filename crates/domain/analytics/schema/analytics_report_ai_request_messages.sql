CREATE TABLE IF NOT EXISTS analytics_report_ai_request_messages (
    id TEXT PRIMARY KEY,
    request_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_ai_request_messages_request_id ON analytics_report_ai_request_messages (request_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_request_messages_created_at ON analytics_report_ai_request_messages (created_at);
