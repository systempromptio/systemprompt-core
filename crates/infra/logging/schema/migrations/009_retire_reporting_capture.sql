-- The reporting projection is retired: its capture triggers and source views
-- on this extension's tables go, with the privacy routine that locked them.
DROP TRIGGER IF EXISTS reporting_capture_insert ON analytics_events;
DROP TRIGGER IF EXISTS reporting_capture_update ON analytics_events;
DROP TRIGGER IF EXISTS reporting_capture_delete ON analytics_events;
DROP TRIGGER IF EXISTS reporting_capture ON analytics_events;
DROP VIEW IF EXISTS reporting_source_analytics_events;
DROP FUNCTION IF EXISTS public.lock_logging_reporting_sources();
