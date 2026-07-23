-- Unconsumed link-analytics views removed; campaign_links / link_clicks
-- remain the queryable source tables.
DROP VIEW IF EXISTS v_link_performance CASCADE;
DROP VIEW IF EXISTS v_campaign_performance CASCADE;
DROP VIEW IF EXISTS v_content_journey CASCADE;
DROP VIEW IF EXISTS v_link_click_stream CASCADE;
DROP VIEW IF EXISTS v_top_performing_links CASCADE;
DROP VIEW IF EXISTS v_link_performance_by_device CASCADE;
DROP VIEW IF EXISTS v_link_performance_by_country CASCADE;
DROP VIEW IF EXISTS v_campaign_daily_performance CASCADE;
DROP VIEW IF EXISTS v_source_content_performance CASCADE;
DROP INDEX IF EXISTS idx_link_clicks_device_type;
DROP INDEX IF EXISTS idx_link_clicks_country;
DROP INDEX IF EXISTS idx_link_clicks_date;
DROP INDEX IF EXISTS idx_link_clicks_referrer_page;
