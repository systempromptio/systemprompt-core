-- This crate's share of the trace facts projection (database schema
-- trace_facts.sql): the per-trace aggregate of its AI requests, and the
-- statement triggers that queue a trace whenever one of its requests is
-- inserted, settled, moved or deleted.
CREATE OR REPLACE FUNCTION trace_facts_ai(keys TEXT[])
RETURNS TABLE(trace_id TEXT, first_at TIMESTAMPTZ, last_at TIMESTAMPTZ, row_count BIGINT,
              running BOOLEAN, failed BOOLEAN)
LANGUAGE plpgsql STABLE AS $$
BEGIN
    RETURN QUERY
    SELECT r.trace_id::text, MIN(r.created_at), MAX(r.created_at), COUNT(*),
           bool_or(r.status = 'pending'), bool_or(r.status = 'failed')
    FROM ai_requests r
    WHERE r.trace_id = ANY(keys)
    GROUP BY r.trace_id;
END $$;

DROP TRIGGER IF EXISTS trace_facts_ai_insert ON ai_requests;
CREATE TRIGGER trace_facts_ai_insert
    AFTER INSERT ON ai_requests
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();

DROP TRIGGER IF EXISTS trace_facts_ai_update ON ai_requests;
CREATE TRIGGER trace_facts_ai_update
    AFTER UPDATE ON ai_requests
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();

DROP TRIGGER IF EXISTS trace_facts_ai_delete ON ai_requests;
CREATE TRIGGER trace_facts_ai_delete
    AFTER DELETE ON ai_requests
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_rows();
