-- This crate's share of the trace facts projection (database schema
-- trace_facts.sql): the per-trace aggregate of its tasks (the agent is the
-- one on the most recently updated task), and the statement triggers that
-- queue a trace whenever one of its tasks is inserted, changes state, moves
-- or is deleted. A task with an empty trace id belongs to no trace.
CREATE OR REPLACE FUNCTION trace_facts_agent(keys TEXT[])
RETURNS TABLE(trace_id TEXT, first_at TIMESTAMPTZ, last_at TIMESTAMPTZ, row_count BIGINT,
              running BOOLEAN, failed BOOLEAN, canceled BOOLEAN, agent_name TEXT)
LANGUAGE plpgsql STABLE AS $$
BEGIN
    RETURN QUERY
    SELECT t.trace_id, MIN(t.created_at), MAX(t.created_at), COUNT(*),
           bool_or(t.status IN ('TASK_STATE_PENDING', 'TASK_STATE_SUBMITTED', 'TASK_STATE_WORKING',
                                'TASK_STATE_INPUT_REQUIRED', 'TASK_STATE_AUTH_REQUIRED')),
           bool_or(t.status IN ('TASK_STATE_FAILED', 'TASK_STATE_REJECTED')),
           bool_or(t.status = 'TASK_STATE_CANCELED'),
           (array_agg(t.agent_name ORDER BY t.updated_at DESC))[1]
    FROM agent_tasks t
    WHERE t.trace_id = ANY(keys) AND t.trace_id <> ''
    GROUP BY t.trace_id;
END $$;

DROP TRIGGER IF EXISTS trace_facts_agent_insert ON agent_tasks;
CREATE TRIGGER trace_facts_agent_insert
    AFTER INSERT ON agent_tasks
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();

DROP TRIGGER IF EXISTS trace_facts_agent_update ON agent_tasks;
CREATE TRIGGER trace_facts_agent_update
    AFTER UPDATE ON agent_tasks
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();

DROP TRIGGER IF EXISTS trace_facts_agent_delete ON agent_tasks;
CREATE TRIGGER trace_facts_agent_delete
    AFTER DELETE ON agent_tasks
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();
