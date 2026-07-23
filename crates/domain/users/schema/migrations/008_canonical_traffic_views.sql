-- Canonical traffic taxonomy: v_clean_traffic / v_engaged_traffic become the
-- single human-traffic definition (all four bot flags excluded). The redundant
-- v_clean_human_traffic (dead is_behavioral_bot NULL-guard on a NOT NULL
-- column) is removed, and the partial indexes are rebuilt to match the view
-- predicates exactly (CREATE INDEX IF NOT EXISTS never updates an existing
-- index definition).
-- Unconsumed analytics views are removed outright: v_clean_traffic /
-- v_engaged_traffic / v_bot_sessions (base schema) are the only session
-- analytics surface.
DROP VIEW IF EXISTS v_clean_human_traffic CASCADE;
DROP VIEW IF EXISTS v_session_analytics_by_client CASCADE;
DROP VIEW IF EXISTS v_client_rate_limits CASCADE;
DROP VIEW IF EXISTS v_client_conversion_rates CASCADE;
DROP VIEW IF EXISTS v_scanner_activity CASCADE;
DROP VIEW IF EXISTS v_ai_crawler_activity CASCADE;
DROP VIEW IF EXISTS v_security_threats CASCADE;
DROP VIEW IF EXISTS v_top_referrer_sources CASCADE;
DROP VIEW IF EXISTS v_utm_campaign_performance CASCADE;
DROP VIEW IF EXISTS v_behavioral_bot_analysis CASCADE;
DROP VIEW IF EXISTS v_daily_conversions CASCADE;
DROP VIEW IF EXISTS v_time_to_conversion CASCADE;
DROP VIEW IF EXISTS v_landing_page_conversion CASCADE;
DROP VIEW IF EXISTS v_referrer_landing_flow CASCADE;
DROP VIEW IF EXISTS v_traffic_source_quality CASCADE;
DROP VIEW IF EXISTS v_bot_traffic_summary CASCADE;
DROP VIEW IF EXISTS v_bot_type_breakdown CASCADE;
DROP VIEW IF EXISTS v_traffic_composition CASCADE;
DROP VIEW IF EXISTS v_seo_crawler_activity CASCADE;
DROP VIEW IF EXISTS v_ai_scraper_activity CASCADE;
DROP VIEW IF EXISTS v_bot_human_metrics_comparison CASCADE;
DROP VIEW IF EXISTS v_recent_bot_activity CASCADE;
DROP INDEX IF EXISTS idx_user_sessions_clean_human_traffic;
DROP INDEX IF EXISTS idx_user_sessions_clean_traffic;
DROP INDEX IF EXISTS idx_user_sessions_engaged_traffic;
