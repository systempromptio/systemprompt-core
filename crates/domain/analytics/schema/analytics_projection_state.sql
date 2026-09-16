CREATE TABLE IF NOT EXISTS analytics_projection_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    generation BIGINT NOT NULL DEFAULT 0,
    cutoff_revision BIGINT NOT NULL DEFAULT 0,
    initialized BOOLEAN NOT NULL DEFAULT FALSE,
    rebuilt_at TIMESTAMPTZ,
    evidence_cutoff TIMESTAMPTZ
);
