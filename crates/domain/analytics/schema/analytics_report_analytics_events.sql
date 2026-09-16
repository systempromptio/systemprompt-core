CREATE TABLE IF NOT EXISTS analytics_report_analytics_events (
    id TEXT PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    session_id TEXT,
    context_id VARCHAR(255),
    gateway_conversation_id VARCHAR(255),
    provider_request_id VARCHAR(255),
    event_type VARCHAR(255) NOT NULL,
    event_category TEXT NOT NULL,
    severity TEXT NOT NULL,
    endpoint TEXT,
    error_code INTEGER,
    response_time_ms INTEGER,
    agent_id VARCHAR(255),
    task_id VARCHAR(255),
    message TEXT,
    metadata TEXT,
    event_data JSONB,
    timestamp TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_analytics_events_user_id ON analytics_report_analytics_events (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_analytics_events_session_id ON analytics_report_analytics_events (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_analytics_events_context_id ON analytics_report_analytics_events (context_id);
CREATE INDEX IF NOT EXISTS idx_ar_analytics_events_endpoint ON analytics_report_analytics_events (endpoint);
CREATE INDEX IF NOT EXISTS idx_ar_analytics_events_task_id ON analytics_report_analytics_events (task_id);
CREATE INDEX IF NOT EXISTS idx_ar_analytics_events_timestamp ON analytics_report_analytics_events (timestamp);

CREATE OR REPLACE VIEW analytics_report_v_clean_traffic AS
SELECT * FROM analytics_report_user_sessions
WHERE is_bot = false
  AND is_ai_crawler = false
  AND is_scanner = false
  AND is_behavioral_bot = false;

COMMENT ON VIEW analytics_report_v_clean_traffic IS 'Canonical human traffic: excludes every bot classification (is_bot, is_ai_crawler, is_scanner, is_behavioral_bot)';

CREATE OR REPLACE VIEW analytics_report_v_engaged_traffic AS
SELECT * FROM analytics_report_user_sessions
WHERE is_bot = false
  AND is_ai_crawler = false
  AND is_scanner = false
  AND is_behavioral_bot = false
  AND landing_page IS NOT NULL
  AND request_count > 0;

COMMENT ON VIEW analytics_report_v_engaged_traffic IS 'Human traffic with actual page engagement (excludes ghost sessions with no landing page or zero requests)';

CREATE INDEX IF NOT EXISTS idx_ar_sessions_engaged_traffic
ON analytics_report_user_sessions(started_at)
WHERE is_bot = false
  AND is_ai_crawler = false
  AND is_scanner = false
  AND is_behavioral_bot = false
  AND landing_page IS NOT NULL
  AND request_count > 0;

CREATE OR REPLACE VIEW analytics_report_v_bot_sessions AS
SELECT
    *,
    CASE
        WHEN user_agent ILIKE '%googlebot%' OR user_agent ILIKE '%google-inspectiontool%' OR user_agent ILIKE '%adsbot-google%' THEN 'Google'
        WHEN user_agent ILIKE '%bingbot%' OR user_agent ILIKE '%bingpreview%' OR user_agent ILIKE '%msnbot%' THEN 'Bing'
        WHEN user_agent ILIKE '%chatgpt%' OR user_agent ILIKE '%gptbot%' THEN 'OpenAI'
        WHEN user_agent ILIKE '%claude%' OR user_agent ILIKE '%anthropic%' THEN 'Anthropic'
        WHEN user_agent ILIKE '%perplexity%' THEN 'Perplexity'
        WHEN user_agent ILIKE '%baiduspider%' THEN 'Baidu'
        WHEN user_agent ILIKE '%yandexbot%' THEN 'Yandex'
        WHEN user_agent ILIKE '%facebookexternalhit%' OR user_agent ILIKE '%facebot%' OR user_agent ILIKE '%meta-externalagent%' THEN 'Meta'
        WHEN user_agent ILIKE '%twitterbot%' THEN 'Twitter/X'
        WHEN user_agent ILIKE '%linkedinbot%' THEN 'LinkedIn'
        WHEN user_agent ILIKE '%semrushbot%' OR user_agent ILIKE '%ahrefsbot%' OR user_agent ILIKE '%mj12bot%' OR user_agent ILIKE '%dotbot%' THEN 'SEO Crawlers'
        WHEN user_agent ILIKE '%bytespider%' THEN 'ByteDance'
        WHEN user_agent ILIKE '%amazonbot%' OR user_agent ILIKE '%applebot%' THEN 'Tech Giants'
        WHEN user_agent ILIKE '%python%' OR user_agent ILIKE '%scrapy%' OR user_agent ILIKE '%httpx%' THEN 'Python Scrapers'
        WHEN user_agent ILIKE '%curl%' OR user_agent ILIKE '%wget%' OR user_agent ILIKE '%node-fetch%' OR user_agent ILIKE '%axios%' THEN 'CLI/HTTP Tools'
        WHEN user_agent ILIKE '%headless%' OR user_agent ILIKE '%phantom%' OR user_agent ILIKE '%selenium%' OR user_agent ILIKE '%puppeteer%' THEN 'Headless Browsers'
        WHEN user_agent ILIKE '%uptimerobot%' OR user_agent ILIKE '%pingdom%' OR user_agent ILIKE '%statuscake%' OR user_agent ILIKE '%lighthouse%' THEN 'Monitoring'
        WHEN is_ai_crawler = true THEN 'AI Crawler'
        WHEN is_behavioral_bot = true THEN 'Behavioral Bot'
        WHEN is_scanner = true THEN 'Scanner'
        ELSE 'Other'
    END as bot_type
FROM analytics_report_user_sessions
WHERE is_bot = true
   OR is_ai_crawler = true
   OR is_scanner = true
   OR is_behavioral_bot = true;

COMMENT ON VIEW analytics_report_v_bot_sessions IS 'Complement of analytics_report_v_clean_traffic: every session with any bot classification, labelled with the canonical user-agent bot taxonomy (bot_type)';
