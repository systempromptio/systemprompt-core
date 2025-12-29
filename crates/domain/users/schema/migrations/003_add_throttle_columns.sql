-- Migration: Add throttle and behavioral score columns
-- Phase 2: Progressive throttling and behavioral scoring

-- Step 1: Add columns
ALTER TABLE user_sessions
ADD COLUMN IF NOT EXISTS throttle_level INTEGER NOT NULL DEFAULT 0;

ALTER TABLE user_sessions
ADD COLUMN IF NOT EXISTS behavioral_bot_score INTEGER NOT NULL DEFAULT 0;

ALTER TABLE user_sessions
ADD COLUMN IF NOT EXISTS throttle_escalated_at TIMESTAMPTZ;

-- Step 2: Add comments
COMMENT ON COLUMN user_sessions.throttle_level IS 'Rate limit escalation level: 0=Normal, 1=Warning (50%), 2=Severe (25%), 3=Blocked';
COMMENT ON COLUMN user_sessions.behavioral_bot_score IS 'Cumulative behavioral bot score from multi-signal detection (0-100+)';
COMMENT ON COLUMN user_sessions.throttle_escalated_at IS 'Timestamp of last throttle level escalation';

-- Step 3: Add indexes
CREATE INDEX IF NOT EXISTS idx_user_sessions_throttle_level
    ON user_sessions(throttle_level) WHERE throttle_level > 0;
CREATE INDEX IF NOT EXISTS idx_user_sessions_behavioral_score
    ON user_sessions(behavioral_bot_score) WHERE behavioral_bot_score >= 50;

-- Step 4: Create view for clean human traffic
DROP VIEW IF EXISTS v_clean_human_traffic CASCADE;
CREATE VIEW v_clean_human_traffic AS
SELECT * FROM user_sessions
WHERE is_bot = false
  AND is_scanner = false
  AND (is_behavioral_bot IS NULL OR is_behavioral_bot = false)
  AND throttle_level < 3;

COMMENT ON VIEW v_clean_human_traffic IS 'Consolidated view of verified human traffic excluding all bot types and blocked sessions';

CREATE INDEX IF NOT EXISTS idx_user_sessions_clean_human_traffic
ON user_sessions(started_at)
WHERE is_bot = false
  AND is_scanner = false
  AND (is_behavioral_bot IS NULL OR is_behavioral_bot = false)
  AND throttle_level < 3;

-- Step 5: Create behavioral bot analysis view
DROP VIEW IF EXISTS v_behavioral_bot_analysis CASCADE;
CREATE VIEW v_behavioral_bot_analysis AS
SELECT
    DATE(started_at) as date,
    COUNT(*) as total_sessions,
    COUNT(CASE WHEN is_behavioral_bot = true THEN 1 END) as behavioral_bot_sessions,
    COUNT(CASE WHEN behavioral_bot_score >= 50 THEN 1 END) as suspicious_sessions,
    AVG(CASE WHEN behavioral_bot_score > 0 THEN behavioral_bot_score END)::INTEGER as avg_bot_score,
    MAX(behavioral_bot_score) as max_bot_score
FROM user_sessions
WHERE started_at >= CURRENT_DATE - INTERVAL '30 days'
GROUP BY DATE(started_at)
ORDER BY date DESC;
