-- `ai_request_messages` is no longer a reporting source. What the projection
-- needs from it is a count per request, kept on ai_requests by the
-- statement trigger in schema/reporting_capture.sql; this backfills it.
--
-- The row-level reporting capture on ai_requests is dropped first: the
-- declarative phase recreates it in statement form after migrations, and
-- a backfill that fired it per row would queue one fact per request under
-- the previous column contract. The view and trigger on the message table
-- go the same way; the declarative phase no longer defines them.
DROP TRIGGER IF EXISTS reporting_capture ON ai_requests;
DROP TRIGGER IF EXISTS reporting_capture ON ai_request_messages;
DROP VIEW IF EXISTS reporting_source_ai_request_messages;
ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS message_count INTEGER NOT NULL DEFAULT 0;
UPDATE ai_requests r SET message_count = c.n
FROM (SELECT request_id, count(*)::integer AS n FROM ai_request_messages GROUP BY request_id) c
WHERE c.request_id = r.id AND r.message_count <> c.n;
