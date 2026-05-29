-- Drop the unenforced per-session throttle mechanism. throttle_level and
-- throttle_escalated_at were written by behavioural detection but never read on
-- the request path: the throttle-enforcement middleware was never mounted. Bot
-- enforcement is handled by ip_ban + rate_limit + the malicious-IP blacklist job.
DROP INDEX IF EXISTS idx_user_sessions_throttle_level;
DROP INDEX IF EXISTS idx_user_sessions_clean_human_traffic;

DROP VIEW IF EXISTS v_clean_traffic CASCADE;
DROP VIEW IF EXISTS v_clean_human_traffic CASCADE;
DROP VIEW IF EXISTS v_engaged_traffic CASCADE;

ALTER TABLE user_sessions DROP COLUMN IF EXISTS throttle_level;
ALTER TABLE user_sessions DROP COLUMN IF EXISTS throttle_escalated_at;

CREATE OR REPLACE VIEW v_clean_traffic AS
SELECT * FROM user_sessions
WHERE is_bot = false
  AND is_ai_crawler = false
  AND is_scanner = false
  AND is_behavioral_bot = false;

CREATE OR REPLACE VIEW v_clean_human_traffic AS
SELECT * FROM user_sessions
WHERE is_bot = false
  AND is_ai_crawler = false
  AND is_scanner = false
  AND (is_behavioral_bot IS NULL OR is_behavioral_bot = false);

COMMENT ON VIEW v_clean_human_traffic IS 'Consolidated view of verified human traffic excluding all bot types';

CREATE OR REPLACE VIEW v_engaged_traffic AS
SELECT * FROM user_sessions
WHERE is_bot = false
  AND is_ai_crawler = false
  AND is_scanner = false
  AND is_behavioral_bot = false
  AND landing_page IS NOT NULL
  AND request_count > 0;

COMMENT ON VIEW v_engaged_traffic IS 'Human traffic with actual page engagement (excludes ghost sessions with no landing page or zero requests)';

CREATE INDEX IF NOT EXISTS idx_user_sessions_clean_human_traffic
ON user_sessions(started_at)
WHERE is_bot = false
  AND is_scanner = false
  AND (is_behavioral_bot IS NULL OR is_behavioral_bot = false);
