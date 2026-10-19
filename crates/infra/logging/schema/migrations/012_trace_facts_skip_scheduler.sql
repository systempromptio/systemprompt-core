-- Scheduler ticks stop feeding the trace facts projection
-- (schema/trace_facts_logs.sql now queues a `system:scheduler:*` log row
-- only when it is an ERROR). This drops the queued keys and fact rows those
-- ticks already produced: a trace with no request, tool call or agent task
-- whose every log row came from a successful tick.

DELETE FROM projection_backlog b
WHERE b.projection = 'trace_facts'
  AND EXISTS (SELECT 1 FROM logs l WHERE l.trace_id = b.key AND l.client_id LIKE 'system:scheduler:%')
  AND NOT EXISTS (SELECT 1 FROM logs l WHERE l.trace_id = b.key
                  AND (l.client_id IS NULL OR l.client_id NOT LIKE 'system:scheduler:%' OR l.level = 'ERROR'));

DELETE FROM trace_facts f
WHERE f.ai_count = 0 AND f.mcp_count = 0 AND f.agent_name IS NULL AND NOT f.has_error
  AND EXISTS (SELECT 1 FROM logs l WHERE l.trace_id = f.trace_id AND l.client_id LIKE 'system:scheduler:%')
  AND NOT EXISTS (SELECT 1 FROM logs l WHERE l.trace_id = f.trace_id
                  AND (l.client_id IS NULL OR l.client_id NOT LIKE 'system:scheduler:%'));
