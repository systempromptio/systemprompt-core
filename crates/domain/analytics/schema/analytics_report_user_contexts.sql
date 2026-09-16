CREATE TABLE IF NOT EXISTS analytics_report_user_contexts (
    context_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    session_id TEXT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_user_contexts_user_id ON analytics_report_user_contexts (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_user_contexts_session_id ON analytics_report_user_contexts (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_user_contexts_created_at ON analytics_report_user_contexts (created_at);
