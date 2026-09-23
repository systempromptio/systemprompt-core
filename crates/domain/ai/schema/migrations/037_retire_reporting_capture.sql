-- The reporting projection is retired: its capture triggers and source views
-- on this extension's tables go, with the privacy routine that locked them.
DROP TRIGGER IF EXISTS reporting_capture_insert ON ai_requests;
DROP TRIGGER IF EXISTS reporting_capture_update ON ai_requests;
DROP TRIGGER IF EXISTS reporting_capture_delete ON ai_requests;
DROP TRIGGER IF EXISTS reporting_capture ON ai_requests;
DROP VIEW IF EXISTS reporting_source_ai_requests;
DROP FUNCTION IF EXISTS public.lock_ai_reporting_sources();
DROP FUNCTION IF EXISTS public.reporting_request_is_retained(TEXT, TIMESTAMPTZ);
