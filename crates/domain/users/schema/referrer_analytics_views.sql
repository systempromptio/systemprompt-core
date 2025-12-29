-- v_top_referrer_sources is defined in user_sessions.sql with proper bot filters

DROP VIEW IF EXISTS v_landing_page_conversion CASCADE;
CREATE VIEW v_landing_page_conversion AS
SELECT
    landing_page,
    COUNT(*) as total_sessions,
    SUM(CASE WHEN user_type = 'anon' THEN 1 ELSE 0 END) as anonymous_sessions,
    SUM(CASE WHEN user_type = 'registered' THEN 1 ELSE 0 END) as registered_sessions,
    SUM(CASE WHEN converted_at IS NOT NULL THEN 1 ELSE 0 END) as converted_sessions,
    CAST(SUM(CASE WHEN converted_at IS NOT NULL THEN 1 ELSE 0 END) AS NUMERIC) / NULLIF(COUNT(*), 0) * 100 as conversion_rate_percent,
    AVG(request_count) as avg_engagement
FROM user_sessions
WHERE landing_page IS NOT NULL
GROUP BY landing_page
HAVING COUNT(*) >= 5
ORDER BY conversion_rate_percent DESC NULLS LAST;

-- v_utm_campaign_performance is defined in user_sessions.sql with proper bot filters

DROP VIEW IF EXISTS v_referrer_landing_flow CASCADE;
CREATE VIEW v_referrer_landing_flow AS
SELECT
    referrer_source,
    landing_page,
    COUNT(*) as session_count,
    AVG(request_count) as avg_requests,
    AVG(duration_seconds) as avg_duration_seconds,
    SUM(CASE WHEN user_type = 'registered' THEN 1 ELSE 0 END) as registered_users
FROM user_sessions
WHERE referrer_source IS NOT NULL
AND landing_page IS NOT NULL
GROUP BY referrer_source, landing_page
HAVING COUNT(*) >= 3
ORDER BY session_count DESC;

DROP VIEW IF EXISTS v_traffic_source_quality CASCADE;
CREATE VIEW v_traffic_source_quality AS
SELECT
    referrer_source,
    COUNT(*) as sessions,
    AVG(duration_seconds) as avg_duration_seconds,
    AVG(request_count) as avg_requests,
    AVG(ai_request_count) as avg_ai_requests,
    CAST(SUM(CASE WHEN converted_at IS NOT NULL THEN 1 ELSE 0 END) AS NUMERIC) / NULLIF(COUNT(*), 0) * 100 as conversion_rate_percent,
    AVG(success_rate) as avg_success_rate,
    (
        (AVG(duration_seconds) / 60.0 * 0.3) +
        (AVG(request_count) * 0.3) +
        (CAST(SUM(CASE WHEN converted_at IS NOT NULL THEN 1 ELSE 0 END) AS NUMERIC) / NULLIF(COUNT(*), 0) * 100 * 0.4)
    ) as quality_score
FROM user_sessions
WHERE referrer_source IS NOT NULL
GROUP BY referrer_source
HAVING COUNT(*) >= 10
ORDER BY quality_score DESC NULLS LAST;
