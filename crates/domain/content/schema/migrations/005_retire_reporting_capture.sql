-- The reporting projection is retired: its capture triggers and source views
-- on this extension's tables go, with the privacy routine that locked them.
DROP TRIGGER IF EXISTS reporting_capture_insert ON markdown_content;
DROP TRIGGER IF EXISTS reporting_capture_update ON markdown_content;
DROP TRIGGER IF EXISTS reporting_capture_delete ON markdown_content;
DROP TRIGGER IF EXISTS reporting_capture ON markdown_content;
DROP VIEW IF EXISTS reporting_source_markdown_content;
DROP FUNCTION IF EXISTS public.lock_content_reporting_sources();
