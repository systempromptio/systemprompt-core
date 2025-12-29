CREATE TABLE IF NOT EXISTS fingerprint_reputation (
    fingerprint_hash TEXT PRIMARY KEY,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    total_session_count INTEGER NOT NULL DEFAULT 0,
    active_session_count INTEGER NOT NULL DEFAULT 0,
    total_request_count BIGINT NOT NULL DEFAULT 0,
    requests_last_hour INTEGER NOT NULL DEFAULT 0,
    peak_requests_per_minute REAL NOT NULL DEFAULT 0,
    sustained_high_velocity_minutes INTEGER NOT NULL DEFAULT 0,
    is_flagged BOOLEAN NOT NULL DEFAULT FALSE,
    flag_reason TEXT,
    flagged_at TIMESTAMPTZ,
    reputation_score INTEGER NOT NULL DEFAULT 50,
    abuse_incidents INTEGER NOT NULL DEFAULT 0,
    last_abuse_at TIMESTAMPTZ,
    last_ip_address TEXT,
    last_user_agent TEXT,
    associated_user_ids TEXT[] NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_fingerprint_reputation_flagged
    ON fingerprint_reputation(is_flagged) WHERE is_flagged = TRUE;
CREATE INDEX IF NOT EXISTS idx_fingerprint_reputation_score
    ON fingerprint_reputation(reputation_score);
CREATE INDEX IF NOT EXISTS idx_fingerprint_reputation_session_count
    ON fingerprint_reputation(total_session_count);
CREATE INDEX IF NOT EXISTS idx_fingerprint_reputation_last_seen
    ON fingerprint_reputation(last_seen_at);
CREATE INDEX IF NOT EXISTS idx_fingerprint_reputation_abuse
    ON fingerprint_reputation(abuse_incidents) WHERE abuse_incidents > 0;

CREATE OR REPLACE VIEW v_high_risk_fingerprints AS
SELECT
    fingerprint_hash,
    total_session_count,
    requests_last_hour,
    peak_requests_per_minute,
    reputation_score,
    abuse_incidents,
    flag_reason,
    last_ip_address
FROM fingerprint_reputation
WHERE is_flagged = TRUE
   OR reputation_score < 30
   OR abuse_incidents >= 3
ORDER BY reputation_score ASC, abuse_incidents DESC;
