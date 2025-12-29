-- Phase 1: Add behavioral bot detection columns
-- Run this migration to add behavioral bot tracking to existing user_sessions table

-- Step 1: Add columns
ALTER TABLE user_sessions
ADD COLUMN IF NOT EXISTS is_behavioral_bot BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE user_sessions
ADD COLUMN IF NOT EXISTS behavioral_bot_reason TEXT;

-- Step 2: Add comments
COMMENT ON COLUMN user_sessions.is_behavioral_bot IS 'Whether this session exhibits bot-like behavior based on request patterns (high request count, page coverage, etc.)';
COMMENT ON COLUMN user_sessions.behavioral_bot_reason IS 'Reason for behavioral bot classification (e.g., request_count_exceeded, high_page_coverage)';

-- Step 3: Add index for is_behavioral_bot
CREATE INDEX IF NOT EXISTS idx_user_sessions_is_behavioral_bot ON user_sessions(is_behavioral_bot);

-- Step 4: Update clean traffic index to include behavioral bot filter
DROP INDEX IF EXISTS idx_user_sessions_clean_traffic;
CREATE INDEX idx_user_sessions_clean_traffic ON user_sessions(started_at)
WHERE is_bot = false AND is_scanner = false AND is_behavioral_bot = false;

-- Step 5: Recreate views to include behavioral bot filter
DROP VIEW IF EXISTS v_session_analytics_by_client CASCADE;
CREATE VIEW v_session_analytics_by_client AS
SELECT
    client_id,
    client_type,
    COUNT(*) as session_count,
    COUNT(DISTINCT user_id) as unique_users,
    SUM(request_count) as total_requests,
    AVG(duration_seconds) as avg_session_duration_seconds,
    AVG(avg_response_time_ms) as avg_response_time_ms,
    SUM(total_tokens_used) as total_tokens,
    SUM(total_ai_cost_cents) as total_cost_cents,
    MIN(started_at) as first_seen,
    MAX(last_activity_at) as last_seen
FROM user_sessions
WHERE client_type != 'system'
  AND is_bot = false
  AND is_behavioral_bot = false
GROUP BY client_id, client_type
ORDER BY session_count DESC;

DROP VIEW IF EXISTS v_client_rate_limits CASCADE;
CREATE VIEW v_client_rate_limits AS
SELECT
    client_id,
    client_type,
    COUNT(*) as sessions_last_hour,
    MAX(started_at) as last_session_created
FROM user_sessions
WHERE started_at >= NOW() - INTERVAL '1 hour'
  AND is_bot = false
  AND is_behavioral_bot = false
GROUP BY client_id, client_type;

DROP VIEW IF EXISTS v_client_conversion_rates CASCADE;
CREATE VIEW v_client_conversion_rates AS
SELECT
    client_id,
    client_type,
    COUNT(*) as total_sessions,
    SUM(CASE WHEN converted_at IS NOT NULL THEN 1 ELSE 0 END) as converted_sessions,
    CAST(SUM(CASE WHEN converted_at IS NOT NULL THEN 1 ELSE 0 END) AS DOUBLE PRECISION) / COUNT(*) as conversion_rate
FROM user_sessions
WHERE user_type = 'anon'
  AND is_bot = false
  AND is_behavioral_bot = false
GROUP BY client_id, client_type;

DROP VIEW IF EXISTS v_clean_traffic CASCADE;
CREATE VIEW v_clean_traffic AS
SELECT * FROM user_sessions
WHERE is_bot = false
  AND is_scanner = false
  AND is_behavioral_bot = false;

DROP VIEW IF EXISTS v_top_referrer_sources CASCADE;
CREATE VIEW v_top_referrer_sources AS
SELECT
    referrer_source,
    COUNT(*) as session_count,
    COUNT(DISTINCT user_id) as unique_users,
    AVG(request_count) as avg_requests_per_session,
    AVG(duration_seconds) as avg_session_duration_seconds,
    SUM(total_ai_cost_cents) as total_cost_cents
FROM user_sessions
WHERE referrer_source IS NOT NULL
  AND is_bot = false
  AND is_behavioral_bot = false
GROUP BY referrer_source
ORDER BY session_count DESC;

DROP VIEW IF EXISTS v_utm_campaign_performance CASCADE;
CREATE VIEW v_utm_campaign_performance AS
SELECT
    utm_source,
    utm_medium,
    utm_campaign,
    COUNT(*) as session_count,
    COUNT(DISTINCT user_id) as unique_users,
    SUM(CASE WHEN user_type = 'registered' THEN 1 ELSE 0 END) as registered_users,
    SUM(CASE WHEN converted_at IS NOT NULL THEN 1 ELSE 0 END) as conversions,
    CAST(SUM(CASE WHEN converted_at IS NOT NULL THEN 1 ELSE 0 END) AS NUMERIC) / NULLIF(COUNT(*), 0) * 100 as conversion_rate_percent,
    AVG(duration_seconds) as avg_session_duration_seconds,
    SUM(total_ai_cost_cents) as total_cost_cents,
    AVG(total_ai_cost_cents) as avg_cost_per_session_cents
FROM user_sessions
WHERE utm_source IS NOT NULL
  AND is_bot = false
  AND is_behavioral_bot = false
GROUP BY utm_source, utm_medium, utm_campaign
ORDER BY session_count DESC;
