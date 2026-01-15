CREATE TABLE IF NOT EXISTS engagement_events (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
    session_id TEXT NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    page_url TEXT NOT NULL,
    content_id TEXT,

    time_on_page_ms INTEGER NOT NULL DEFAULT 0,
    time_to_first_interaction_ms INTEGER,
    time_to_first_scroll_ms INTEGER,

    max_scroll_depth INTEGER NOT NULL DEFAULT 0,
    scroll_velocity_avg REAL,
    scroll_direction_changes INTEGER DEFAULT 0,

    click_count INTEGER NOT NULL DEFAULT 0,
    mouse_move_distance_px INTEGER DEFAULT 0,
    keyboard_events INTEGER DEFAULT 0,
    copy_events INTEGER DEFAULT 0,

    focus_time_ms INTEGER NOT NULL DEFAULT 0,
    blur_count INTEGER NOT NULL DEFAULT 0,
    tab_switches INTEGER NOT NULL DEFAULT 0,

    visible_time_ms INTEGER NOT NULL DEFAULT 0,
    hidden_time_ms INTEGER NOT NULL DEFAULT 0,

    is_rage_click BOOLEAN DEFAULT FALSE,
    is_dead_click BOOLEAN DEFAULT FALSE,
    reading_pattern VARCHAR(50),

    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT engagement_events_session_fkey
        FOREIGN KEY (session_id)
        REFERENCES user_sessions(session_id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_engagement_events_session
    ON engagement_events(session_id);
CREATE INDEX IF NOT EXISTS idx_engagement_events_user
    ON engagement_events(user_id);
CREATE INDEX IF NOT EXISTS idx_engagement_events_created
    ON engagement_events(created_at);
CREATE INDEX IF NOT EXISTS idx_engagement_events_scroll_depth
    ON engagement_events(max_scroll_depth);
CREATE INDEX IF NOT EXISTS idx_engagement_events_time_on_page
    ON engagement_events(time_on_page_ms);
CREATE INDEX IF NOT EXISTS idx_engagement_events_content_id
    ON engagement_events(content_id);
