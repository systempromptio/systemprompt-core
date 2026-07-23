-- Unconsumed session-analytics views removed; v_clean_traffic /
-- v_engaged_traffic / v_bot_sessions (users extension) are the only
-- session analytics surface.
DROP VIEW IF EXISTS v_preconversion_engagement CASCADE;
DROP VIEW IF EXISTS v_conversion_funnel CASCADE;
DROP VIEW IF EXISTS v_active_anonymous_sessions CASCADE;
