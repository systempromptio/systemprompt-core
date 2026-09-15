CREATE TABLE IF NOT EXISTS analytics_projection_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    generation BIGINT NOT NULL DEFAULT 0,
    cutoff_revision BIGINT NOT NULL DEFAULT 0,
    initialized BOOLEAN NOT NULL DEFAULT FALSE,
    rebuilt_at TIMESTAMPTZ,
    evidence_cutoff TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS analytics_projection_revisions (
    source TEXT NOT NULL,
    entity_key TEXT NOT NULL,
    revision BIGINT NOT NULL,
    PRIMARY KEY (source, entity_key)
);

CREATE TABLE IF NOT EXISTS analytics_report_users (
    id TEXT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    status TEXT NOT NULL,
    roles TEXT[] NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_users_created_at ON analytics_report_users (created_at);

CREATE TABLE IF NOT EXISTS analytics_report_user_sessions (
    session_id TEXT PRIMARY KEY,
    user_id VARCHAR(255),
    started_at TIMESTAMPTZ NOT NULL,
    last_activity_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    duration_seconds INTEGER,
    user_type VARCHAR(255),
    converted_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    client_id VARCHAR(255) NOT NULL,
    client_type VARCHAR(255) NOT NULL,
    request_count INTEGER NOT NULL,
    avg_response_time_ms DOUBLE PRECISION NOT NULL,
    success_rate DOUBLE PRECISION NOT NULL,
    error_count INTEGER NOT NULL,
    task_count INTEGER NOT NULL,
    message_count INTEGER NOT NULL,
    ai_request_count INTEGER NOT NULL,
    total_tokens_used INTEGER NOT NULL,
    total_ai_cost_microdollars BIGINT NOT NULL,
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
    endpoints_accessed TEXT,
    fingerprint_hash TEXT,
    is_bot BOOLEAN NOT NULL,
    is_ai_crawler BOOLEAN NOT NULL,
    is_scanner BOOLEAN NOT NULL,
    is_behavioral_bot BOOLEAN NOT NULL,
    behavioral_bot_reason TEXT,
    behavioral_bot_score INTEGER NOT NULL,
    session_source VARCHAR(50),
    revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_ar_user_sessions_user_id ON analytics_report_user_sessions (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_user_sessions_started_at ON analytics_report_user_sessions (started_at);
CREATE INDEX IF NOT EXISTS idx_ar_user_sessions_last_activity_at ON analytics_report_user_sessions (last_activity_at);
CREATE INDEX IF NOT EXISTS idx_ar_user_sessions_fingerprint_hash ON analytics_report_user_sessions (fingerprint_hash);

CREATE TABLE IF NOT EXISTS analytics_report_agent_tasks (
    task_id TEXT PRIMARY KEY,
    context_id TEXT NOT NULL,
    status TEXT NOT NULL,
    status_timestamp TIMESTAMPTZ,
    user_id TEXT,
    session_id TEXT,
    trace_id TEXT,
    agent_name TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    execution_time_ms INTEGER,
    error_message TEXT,
    version BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_context_id ON analytics_report_agent_tasks (context_id);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_user_id ON analytics_report_agent_tasks (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_session_id ON analytics_report_agent_tasks (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_started_at ON analytics_report_agent_tasks (started_at);
CREATE INDEX IF NOT EXISTS idx_ar_agent_tasks_created_at ON analytics_report_agent_tasks (created_at);

CREATE TABLE IF NOT EXISTS analytics_report_task_messages (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_task_messages_task_id ON analytics_report_task_messages (task_id);
CREATE INDEX IF NOT EXISTS idx_ar_task_messages_created_at ON analytics_report_task_messages (created_at);

CREATE TABLE IF NOT EXISTS analytics_report_user_contexts (
    context_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    session_id TEXT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_user_contexts_user_id ON analytics_report_user_contexts (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_user_contexts_session_id ON analytics_report_user_contexts (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_user_contexts_created_at ON analytics_report_user_contexts (created_at);

CREATE TABLE IF NOT EXISTS analytics_report_ai_requests (
    id TEXT PRIMARY KEY,
    request_id VARCHAR(255) NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    session_id VARCHAR(255),
    task_id TEXT,
    context_id VARCHAR(255) NOT NULL,
    gateway_conversation_id VARCHAR(255),
    client_session_id TEXT,
    provider_request_id VARCHAR(255),
    trace_id VARCHAR(255),
    mcp_execution_id VARCHAR(255),
    provider TEXT,
    model TEXT,
    requested_model TEXT,
    route_match TEXT,
    temperature DOUBLE PRECISION,
    top_p DOUBLE PRECISION,
    max_tokens INTEGER,
    tokens_used INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_microdollars BIGINT NOT NULL,
    latency_ms INTEGER,
    upstream_latency_ms INTEGER,
    cache_hit BOOLEAN NOT NULL,
    cache_read_tokens INTEGER,
    cache_creation_tokens INTEGER,
    reasoning_tokens INTEGER,
    is_streaming BOOLEAN NOT NULL,
    status VARCHAR(255) NOT NULL,
    error_message TEXT,
    actor_kind TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    synthetic BOOLEAN NOT NULL,
    request_kind TEXT NOT NULL,
    instance_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_request_id ON analytics_report_ai_requests (request_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_user_id ON analytics_report_ai_requests (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_session_id ON analytics_report_ai_requests (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_task_id ON analytics_report_ai_requests (task_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_context_id ON analytics_report_ai_requests (context_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_requests_created_at ON analytics_report_ai_requests (created_at);

CREATE TABLE IF NOT EXISTS analytics_report_ai_request_messages (
    id TEXT PRIMARY KEY,
    request_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_ai_request_messages_request_id ON analytics_report_ai_request_messages (request_id);
CREATE INDEX IF NOT EXISTS idx_ar_ai_request_messages_created_at ON analytics_report_ai_request_messages (created_at);

CREATE TABLE IF NOT EXISTS analytics_report_mcp_tool_executions (
    mcp_execution_id TEXT PRIMARY KEY,
    tool_name VARCHAR(255) NOT NULL,
    server_name VARCHAR(255) NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    execution_time_ms INTEGER,
    status VARCHAR(255) NOT NULL,
    error_message TEXT,
    user_id VARCHAR(255) NOT NULL,
    session_id VARCHAR(255),
    context_id VARCHAR(255),
    task_id VARCHAR(255),
    trace_id VARCHAR(255),
    request_method TEXT,
    request_source TEXT,
    actor_kind TEXT,
    actor_id TEXT,
    ai_tool_call_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_started_at ON analytics_report_mcp_tool_executions (started_at);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_user_id ON analytics_report_mcp_tool_executions (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_session_id ON analytics_report_mcp_tool_executions (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_context_id ON analytics_report_mcp_tool_executions (context_id);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_task_id ON analytics_report_mcp_tool_executions (task_id);
CREATE INDEX IF NOT EXISTS idx_ar_mcp_tool_executions_created_at ON analytics_report_mcp_tool_executions (created_at);

CREATE TABLE IF NOT EXISTS analytics_report_markdown_content (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    source_id TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS analytics_report_logs (
    id TEXT PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    level VARCHAR(50) NOT NULL,
    module VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    user_id VARCHAR(255),
    session_id VARCHAR(255),
    task_id VARCHAR(255)
);
CREATE INDEX IF NOT EXISTS idx_ar_logs_timestamp ON analytics_report_logs (timestamp);
CREATE INDEX IF NOT EXISTS idx_ar_logs_user_id ON analytics_report_logs (user_id);
CREATE INDEX IF NOT EXISTS idx_ar_logs_session_id ON analytics_report_logs (session_id);
CREATE INDEX IF NOT EXISTS idx_ar_logs_task_id ON analytics_report_logs (task_id);

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
