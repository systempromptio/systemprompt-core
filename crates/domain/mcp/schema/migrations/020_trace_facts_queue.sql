-- Trace facts (database schema trace_facts.sql): queues the trace of every
-- tool execution that predates the projection; `trace_facts_rollup` drains it.
INSERT INTO projection_backlog (projection, key)
SELECT 'trace_facts', trace_id FROM mcp_tool_executions WHERE trace_id IS NOT NULL AND trace_id <> ''
ON CONFLICT DO NOTHING;
