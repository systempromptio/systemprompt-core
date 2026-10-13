-- This crate's share of the trace facts projection (database schema
-- trace_facts.sql): the per-trace span and error flag of its log rows, and
-- the statement triggers that queue a trace whenever a log row carrying it
-- is written or deleted (log rows are never updated in place).
CREATE OR REPLACE FUNCTION trace_facts_logs(keys TEXT[])
RETURNS TABLE(trace_id TEXT, first_at TIMESTAMPTZ, last_at TIMESTAMPTZ, has_error BOOLEAN)
LANGUAGE plpgsql STABLE AS $$
BEGIN
    RETURN QUERY
    SELECT l.trace_id::text, MIN(l.timestamp), MAX(l.timestamp), bool_or(l.level = 'ERROR')
    FROM logs l
    WHERE l.trace_id = ANY(keys)
    GROUP BY l.trace_id;
END $$;

DROP TRIGGER IF EXISTS trace_facts_logs_insert ON logs;
CREATE TRIGGER trace_facts_logs_insert
    AFTER INSERT ON logs
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();

DROP TRIGGER IF EXISTS trace_facts_logs_delete ON logs;
CREATE TRIGGER trace_facts_logs_delete
    AFTER DELETE ON logs
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();
