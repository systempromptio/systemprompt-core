CREATE TABLE IF NOT EXISTS funnel_progress (
    id TEXT PRIMARY KEY,
    funnel_id TEXT NOT NULL REFERENCES funnels(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    current_step INTEGER NOT NULL DEFAULT 0,
    completed_at TIMESTAMPTZ,
    dropped_at_step INTEGER,
    step_timestamps JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_funnel_progress_funnel_id ON funnel_progress(funnel_id);
CREATE INDEX IF NOT EXISTS idx_funnel_progress_session_id ON funnel_progress(session_id);
CREATE INDEX IF NOT EXISTS idx_funnel_progress_created_at ON funnel_progress(created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_funnel_progress_unique ON funnel_progress(funnel_id, session_id);
