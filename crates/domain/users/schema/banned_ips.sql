CREATE TABLE IF NOT EXISTS banned_ips (
    ip_address VARCHAR(45) PRIMARY KEY,
    reason VARCHAR(255) NOT NULL,
    banned_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ,
    ban_count INTEGER NOT NULL DEFAULT 1,
    last_offense_path VARCHAR(512),
    last_user_agent TEXT,
    is_permanent BOOLEAN NOT NULL DEFAULT FALSE,
    -- Phase 3: Fingerprint association and source tracking
    source_fingerprint TEXT,
    ban_source VARCHAR(50) DEFAULT 'manual',
    associated_session_ids TEXT[] DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_banned_ips_expires ON banned_ips(expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_banned_ips_banned_at ON banned_ips(banned_at);
CREATE INDEX IF NOT EXISTS idx_banned_ips_fingerprint ON banned_ips(source_fingerprint) WHERE source_fingerprint IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_banned_ips_source ON banned_ips(ban_source);

COMMENT ON COLUMN banned_ips.source_fingerprint IS 'Fingerprint that triggered the ban';
COMMENT ON COLUMN banned_ips.ban_source IS 'Source: manual, behavioral_analysis, velocity_abuse, fingerprint_abuse';
