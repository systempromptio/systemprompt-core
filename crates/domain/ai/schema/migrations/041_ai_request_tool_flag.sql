-- Offered-tools flag (schema/ai_request_tool_flag.sql): whether a request
-- offered client tools is stored on the request row instead of read from the
-- payload ledger by every conversation page. The column is the base schema's
-- text; the requests that predate it are queued for
-- `refresh_ai_request_tool_flag`, which the `ai_request_tool_flag_rollup`
-- job drains.
ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS has_offered_tools BOOLEAN;

INSERT INTO projection_backlog (projection, key)
SELECT 'ai_request_tool_flag', id FROM ai_requests WHERE has_offered_tools IS NULL
ON CONFLICT DO NOTHING;
