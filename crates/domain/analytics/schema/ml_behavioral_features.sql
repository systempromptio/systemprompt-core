CREATE TABLE IF NOT EXISTS ml_behavioral_features (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
    session_id TEXT NOT NULL,
    fingerprint_hash TEXT,

    is_bot BOOLEAN,
    is_human_verified BOOLEAN DEFAULT FALSE,
    label_source VARCHAR(50),

    session_duration_seconds INTEGER,
    total_requests INTEGER,
    unique_pages_visited INTEGER,
    avg_time_between_requests_ms INTEGER,
    request_time_variance REAL,

    referrer_present BOOLEAN,
    has_javascript BOOLEAN,
    accepts_cookies BOOLEAN,
    viewport_width INTEGER,
    viewport_height INTEGER,

    avg_scroll_depth REAL,
    max_scroll_depth INTEGER,
    avg_time_on_page_ms INTEGER,
    total_clicks INTEGER,
    avg_mouse_speed REAL,
    mouse_movement_entropy REAL,

    time_pattern_regularity REAL,
    request_burst_count INTEGER,

    headless_indicators INTEGER,
    automation_indicators INTEGER,
    fingerprint_anomaly_score REAL,

    feature_vector REAL[] DEFAULT '{}',

    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT ml_features_session_fkey
        FOREIGN KEY (session_id)
        REFERENCES user_sessions(session_id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ml_features_session
    ON ml_behavioral_features(session_id);
CREATE INDEX IF NOT EXISTS idx_ml_features_fingerprint
    ON ml_behavioral_features(fingerprint_hash);
CREATE INDEX IF NOT EXISTS idx_ml_features_labeled
    ON ml_behavioral_features(is_bot, is_human_verified)
    WHERE is_human_verified = TRUE;
CREATE INDEX IF NOT EXISTS idx_ml_features_created
    ON ml_behavioral_features(created_at);
