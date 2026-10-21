-- This crate's share of the trace facts projection (database schema
-- trace_facts.sql): the per-trace span and error flag of its log rows, and
-- the statement triggers that queue a trace whenever a log row carrying it
-- is written or deleted (log rows are never updated in place). A scheduler
-- tick logs under a fresh `system:scheduler:*` trace of its own, so queuing
-- those would make every tick a trace and the drainer feed itself; only an
-- ERROR from a tick is queued.
CREATE OR REPLACE FUNCTION queue_trace_facts_log_rows()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    IF TG_OP = 'INSERT' THEN
        keys := ARRAY(SELECT DISTINCT n.trace_id::text FROM new_rows n
                      WHERE n.trace_id IS NOT NULL AND n.trace_id <> ''
                        AND (n.client_id IS NULL OR n.client_id NOT LIKE 'system:scheduler:%'
                             OR n.level = 'ERROR'));
    ELSE
        keys := ARRAY(SELECT DISTINCT o.trace_id::text FROM old_rows o
                      WHERE o.trace_id IS NOT NULL AND o.trace_id <> ''
                        AND (o.client_id IS NULL OR o.client_id NOT LIKE 'system:scheduler:%'
                             OR o.level = 'ERROR'));
    END IF;
    IF cardinality(keys) > 0 THEN PERFORM queue_projection_backlog('trace_facts', keys); END IF;
    RETURN NULL;
END $$;

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
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_log_rows();

DROP TRIGGER IF EXISTS trace_facts_logs_delete ON logs;
CREATE TRIGGER trace_facts_logs_delete
    AFTER DELETE ON logs
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION queue_trace_facts_log_rows();
