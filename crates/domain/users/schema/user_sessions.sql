CREATE TABLE IF NOT EXISTS user_sessions (
    session_id TEXT PRIMARY KEY,
    user_id VARCHAR(255),
    started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ended_at TIMESTAMPTZ,
    duration_seconds INTEGER,
    user_type VARCHAR(255) DEFAULT 'registered' CHECK (user_type IN ('anon', 'registered')),
    converted_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ DEFAULT (CURRENT_TIMESTAMP + INTERVAL '7 days'),
    client_id VARCHAR(255) NOT NULL DEFAULT 'sp_web',
    client_type VARCHAR(255) NOT NULL DEFAULT 'firstparty' CHECK (client_type IN ('cimd', 'firstparty', 'thirdparty', 'system', 'unknown')),
    request_count INTEGER NOT NULL DEFAULT 0,
    avg_response_time_ms DOUBLE PRECISION NOT NULL DEFAULT 0,
    success_rate DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    error_count INTEGER NOT NULL DEFAULT 0,
    task_count INTEGER NOT NULL DEFAULT 0,
    message_count INTEGER NOT NULL DEFAULT 0,
    ai_request_count INTEGER NOT NULL DEFAULT 0,
    total_tokens_used INTEGER NOT NULL DEFAULT 0,
    total_ai_cost_microdollars BIGINT NOT NULL DEFAULT 0,
    ip_address TEXT,
    user_agent TEXT,
    device_type VARCHAR(255),
    browser TEXT,
    os TEXT,
    country TEXT,
    region TEXT,
    city TEXT,
    preferred_locale TEXT,
    referrer_source VARCHAR(255),
    referrer_url TEXT,
    landing_page TEXT,
    entry_url TEXT,
    utm_source VARCHAR(100),
    utm_medium VARCHAR(100),
    utm_campaign VARCHAR(100),
    utm_content VARCHAR(100),
    utm_term VARCHAR(100),
    endpoints_accessed TEXT DEFAULT '[]',
    fingerprint_hash TEXT,
    is_bot BOOLEAN NOT NULL DEFAULT false,
    is_ai_crawler BOOLEAN NOT NULL DEFAULT false,
    is_scanner BOOLEAN NOT NULL DEFAULT false,
    is_behavioral_bot BOOLEAN NOT NULL DEFAULT false,
    behavioral_bot_reason TEXT,
    behavioral_bot_score INTEGER NOT NULL DEFAULT 0,
    session_source VARCHAR(50) DEFAULT 'web'
        CHECK (session_source IN ('web', 'api', 'cli', 'oauth', 'mcp', 'bridge', 'unknown')),
    revoked_at TIMESTAMPTZ,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
);

COMMENT ON COLUMN user_sessions.behavioral_bot_score IS 'Cumulative behavioral bot score from multi-signal detection (0-100+)';

COMMENT ON COLUMN user_sessions.total_ai_cost_microdollars IS 'AI cost in microdollars (millionths of a dollar). Divide by 1,000,000 to get USD.';
COMMENT ON COLUMN user_sessions.is_bot IS 'Whether this session was created by a UA-matched bot/crawler. Always false on persisted rows in practice: the analytics extractor sets skip_tracking for UA-matched bots, so their sessions are never inserted. Kept so the canonical traffic views state the full predicate.';
COMMENT ON COLUMN user_sessions.is_ai_crawler IS 'Whether this session was created by a declared AI agent/crawler (NotebookLM, ChatGPT-User, ClaudeBot, etc.). Tracked separately from is_bot so AI citations are visible without polluting human metrics.';
COMMENT ON COLUMN user_sessions.is_scanner IS 'Whether this session exhibits scanner/attacker behavior (accessing .php, .env, admin paths, high velocity)';
COMMENT ON COLUMN user_sessions.is_behavioral_bot IS 'Whether this session exhibits bot-like behavior based on request patterns (high request count, page coverage, etc.)';
COMMENT ON COLUMN user_sessions.behavioral_bot_reason IS 'Reason for behavioral bot classification (e.g., request_count_exceeded, high_page_coverage)';
COMMENT ON COLUMN user_sessions.session_source IS 'Origin of the session: web (browser), api (programmatic), cli (command line), oauth (token endpoint)';
CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON user_sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON user_sessions(started_at);
CREATE INDEX IF NOT EXISTS idx_sessions_last_activity ON user_sessions(last_activity_at);
CREATE INDEX IF NOT EXISTS idx_sessions_country ON user_sessions(country);
CREATE INDEX IF NOT EXISTS idx_sessions_device_type ON user_sessions(device_type);
CREATE INDEX IF NOT EXISTS idx_sessions_ai_usage ON user_sessions(ai_request_count);
CREATE INDEX IF NOT EXISTS idx_sessions_cost ON user_sessions(total_ai_cost_microdollars);
CREATE INDEX IF NOT EXISTS idx_sessions_fingerprint ON user_sessions(fingerprint_hash);
CREATE INDEX IF NOT EXISTS idx_sessions_fingerprint_activity ON user_sessions(fingerprint_hash, last_activity_at);
CREATE INDEX IF NOT EXISTS idx_user_sessions_user_type ON user_sessions(user_type);
CREATE INDEX IF NOT EXISTS idx_user_sessions_converted ON user_sessions(converted_at);
CREATE INDEX IF NOT EXISTS idx_user_sessions_expires ON user_sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_user_sessions_client_id ON user_sessions(client_id);
CREATE INDEX IF NOT EXISTS idx_user_sessions_client_type ON user_sessions(client_type);
CREATE INDEX IF NOT EXISTS idx_user_sessions_client_activity ON user_sessions(client_id, last_activity_at);
CREATE INDEX IF NOT EXISTS idx_user_sessions_client_cost ON user_sessions(client_id, total_ai_cost_microdollars);
CREATE INDEX IF NOT EXISTS idx_user_sessions_referrer_source ON user_sessions(referrer_source);
CREATE INDEX IF NOT EXISTS idx_user_sessions_utm_source ON user_sessions(utm_source);
CREATE INDEX IF NOT EXISTS idx_user_sessions_landing_page ON user_sessions(landing_page);
CREATE INDEX IF NOT EXISTS idx_user_sessions_is_bot ON user_sessions(is_bot);
CREATE INDEX IF NOT EXISTS idx_user_sessions_is_ai_crawler ON user_sessions(is_ai_crawler) WHERE is_ai_crawler = true;
CREATE INDEX IF NOT EXISTS idx_user_sessions_human_activity ON user_sessions(is_bot, last_activity_at) WHERE is_bot = false;
CREATE INDEX IF NOT EXISTS idx_user_sessions_bot_activity ON user_sessions(is_bot, started_at) WHERE is_bot = true;
CREATE INDEX IF NOT EXISTS idx_user_sessions_human_sessions ON user_sessions(is_bot, started_at, user_id) WHERE is_bot = false;
CREATE INDEX IF NOT EXISTS idx_user_sessions_is_scanner ON user_sessions(is_scanner);
CREATE INDEX IF NOT EXISTS idx_user_sessions_is_behavioral_bot ON user_sessions(is_behavioral_bot);
CREATE INDEX IF NOT EXISTS idx_user_sessions_behavioral_score ON user_sessions(behavioral_bot_score) WHERE behavioral_bot_score >= 50;
CREATE INDEX IF NOT EXISTS idx_user_sessions_clean_traffic ON user_sessions(started_at) WHERE is_bot = false AND is_ai_crawler = false AND is_scanner = false AND is_behavioral_bot = false;
CREATE INDEX IF NOT EXISTS idx_sessions_referrer ON user_sessions(referrer_source, started_at) WHERE is_bot = false;
CREATE INDEX IF NOT EXISTS idx_sessions_utm ON user_sessions(utm_source, utm_campaign, utm_medium, started_at) WHERE is_bot = false;
CREATE INDEX IF NOT EXISTS idx_sessions_landing ON user_sessions(landing_page, is_bot);
CREATE INDEX IF NOT EXISTS idx_sessions_entry ON user_sessions(entry_url, is_bot);
CREATE INDEX IF NOT EXISTS idx_sessions_engagement ON user_sessions(duration_seconds, request_count, is_bot);
CREATE INDEX IF NOT EXISTS idx_sessions_quality ON user_sessions(success_rate, error_count, is_bot);
CREATE INDEX IF NOT EXISTS idx_sessions_fingerprint_time ON user_sessions(fingerprint_hash, started_at) WHERE is_bot = false;
CREATE INDEX IF NOT EXISTS idx_sessions_user_time ON user_sessions(user_id, started_at) WHERE is_bot = false;
CREATE INDEX IF NOT EXISTS idx_sessions_bot_time ON user_sessions(is_bot, started_at);
CREATE INDEX IF NOT EXISTS idx_sessions_started_bot ON user_sessions(started_at DESC, is_bot);
CREATE INDEX IF NOT EXISTS idx_user_sessions_session_source ON user_sessions(session_source);
CREATE INDEX IF NOT EXISTS idx_user_sessions_revoked ON user_sessions(revoked_at) WHERE revoked_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_user_sessions_visitor_traffic
    ON user_sessions(started_at)
    WHERE session_source = 'web' AND is_bot = false;

-- Views are dropped before recreation: CREATE OR REPLACE VIEW cannot rename or
-- reorder output columns, so an analytics column rename on an existing install
-- would otherwise fail. Views are stateless — dropping loses nothing.
DROP VIEW IF EXISTS v_clean_traffic CASCADE;
DROP VIEW IF EXISTS v_engaged_traffic CASCADE;
DROP VIEW IF EXISTS v_bot_sessions CASCADE;

-- Canonical human-traffic predicate. Every consumer (Rust repositories,
-- downstream extensions) must derive from v_clean_traffic / v_engaged_traffic
-- rather than restating flag combinations.
CREATE OR REPLACE VIEW v_clean_traffic AS
SELECT * FROM user_sessions
WHERE is_bot = false
  AND is_ai_crawler = false
  AND is_scanner = false
  AND is_behavioral_bot = false;

COMMENT ON VIEW v_clean_traffic IS 'Canonical human traffic: excludes every bot classification (is_bot, is_ai_crawler, is_scanner, is_behavioral_bot)';

CREATE OR REPLACE VIEW v_engaged_traffic AS
SELECT * FROM user_sessions
WHERE is_bot = false
  AND is_ai_crawler = false
  AND is_scanner = false
  AND is_behavioral_bot = false
  AND landing_page IS NOT NULL
  AND request_count > 0;

COMMENT ON VIEW v_engaged_traffic IS 'Human traffic with actual page engagement (excludes ghost sessions with no landing page or zero requests)';

CREATE INDEX IF NOT EXISTS idx_user_sessions_engaged_traffic
ON user_sessions(started_at)
WHERE is_bot = false
  AND is_ai_crawler = false
  AND is_scanner = false
  AND is_behavioral_bot = false
  AND landing_page IS NOT NULL
  AND request_count > 0;

CREATE OR REPLACE VIEW v_bot_sessions AS
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
FROM user_sessions
WHERE is_bot = true
   OR is_ai_crawler = true
   OR is_scanner = true
   OR is_behavioral_bot = true;

COMMENT ON VIEW v_bot_sessions IS 'Complement of v_clean_traffic: every session with any bot classification, labelled with the canonical user-agent bot taxonomy (bot_type)';
