CREATE TABLE IF NOT EXISTS funnels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS funnel_steps (
    funnel_id TEXT NOT NULL REFERENCES funnels(id) ON DELETE CASCADE,
    step_order INTEGER NOT NULL,
    name TEXT NOT NULL,
    match_pattern TEXT NOT NULL,
    match_type TEXT NOT NULL DEFAULT 'url_prefix',
    PRIMARY KEY (funnel_id, step_order)
);

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

CREATE INDEX IF NOT EXISTS idx_funnels_active ON funnels(is_active) WHERE is_active = TRUE;
CREATE INDEX IF NOT EXISTS idx_funnel_steps_funnel_id ON funnel_steps(funnel_id);
CREATE INDEX IF NOT EXISTS idx_funnel_progress_funnel_id ON funnel_progress(funnel_id);
CREATE INDEX IF NOT EXISTS idx_funnel_progress_session_id ON funnel_progress(session_id);
CREATE INDEX IF NOT EXISTS idx_funnel_progress_created_at ON funnel_progress(created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_funnel_progress_unique ON funnel_progress(funnel_id, session_id);
