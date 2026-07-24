CREATE TABLE IF NOT EXISTS funnel_steps (
    funnel_id TEXT NOT NULL REFERENCES funnels(id) ON DELETE CASCADE,
    step_order INTEGER NOT NULL,
    name TEXT NOT NULL,
    match_pattern TEXT NOT NULL,
    match_type TEXT NOT NULL DEFAULT 'url_prefix',
    PRIMARY KEY (funnel_id, step_order)
);

CREATE INDEX IF NOT EXISTS idx_funnel_steps_funnel_id ON funnel_steps(funnel_id);
