-- Migration: Add fingerprint association and source tracking to banned_ips
-- Phase 3: Fingerprint Reputation & IP Banning

-- Add fingerprint association columns
ALTER TABLE banned_ips
    ADD COLUMN IF NOT EXISTS source_fingerprint TEXT,
    ADD COLUMN IF NOT EXISTS ban_source VARCHAR(50) DEFAULT 'manual',
    ADD COLUMN IF NOT EXISTS associated_session_ids TEXT[] DEFAULT '{}';

-- Create indexes for new columns
CREATE INDEX IF NOT EXISTS idx_banned_ips_fingerprint
    ON banned_ips(source_fingerprint) WHERE source_fingerprint IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_banned_ips_source
    ON banned_ips(ban_source);

-- Add comments
COMMENT ON COLUMN banned_ips.source_fingerprint IS 'Fingerprint that triggered the ban';
COMMENT ON COLUMN banned_ips.ban_source IS 'Source: manual, behavioral_analysis, velocity_abuse, fingerprint_abuse';
