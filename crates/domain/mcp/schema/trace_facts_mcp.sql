-- This crate's share of the trace facts projection (database schema
-- trace_facts.sql): the per-trace aggregate of its tool calls, and the
-- statement triggers that queue a trace whenever one of its executions is
-- inserted, finished, moved or deleted.
CREATE OR REPLACE FUNCTION trace_facts_mcp(keys TEXT[])
RETURNS TABLE(trace_id TEXT, first_at TIMESTAMPTZ, last_at TIMESTAMPTZ, row_count BIGINT,
              running BOOLEAN, failed BOOLEAN, tool_names TEXT[])
LANGUAGE plpgsql STABLE AS $$
BEGIN
    RETURN QUERY
    SELECT e.trace_id::text, MIN(e.started_at), MAX(e.started_at), COUNT(*),
           bool_or(e.status = 'pending'), bool_or(e.status IN ('failed', 'timeout')),
           array_agg(DISTINCT e.tool_name::text ORDER BY e.tool_name::text)
    FROM mcp_tool_executions e
    WHERE e.trace_id = ANY(keys)
    GROUP BY e.trace_id;
END $$;

DROP TRIGGER IF EXISTS trace_facts_mcp_insert ON mcp_tool_executions;
CREATE TRIGGER trace_facts_mcp_insert
    AFTER INSERT ON mcp_tool_executions
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();

DROP TRIGGER IF EXISTS trace_facts_mcp_update ON mcp_tool_executions;
CREATE TRIGGER trace_facts_mcp_update
    AFTER UPDATE ON mcp_tool_executions
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();

DROP TRIGGER IF EXISTS trace_facts_mcp_delete ON mcp_tool_executions;
CREATE TRIGGER trace_facts_mcp_delete
    AFTER DELETE ON mcp_tool_executions
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();
