-- Trace facts: one row per trace id, so the trace list reads the answer
-- instead of unioning four ledgers and running a dozen correlated EXISTS per
-- trace on every load (store, don't compute).
--
--   first_at / last_at  earliest and latest timestamp of any row carrying the
--                       trace id in ai_requests, mcp_tool_executions,
--                       agent_tasks or logs
--   agent_name          the agent of the trace's most recently updated task
--   status              'running' while a task, AI request or tool call is
--                       pending; else 'failed' when one failed (or, for a
--                       trace that is only logs, when one is an ERROR);
--                       else 'canceled' for a canceled task; else 'completed'
--   ai_count/mcp_count  AI requests and tool calls on the trace
--   has_error           an ERROR log was written on the trace
--   tool_names          the distinct tool names called on the trace
--
-- Each ledger's owner contributes one set-returning function over its own
-- table (`trace_facts_ai`, `trace_facts_mcp`, `trace_facts_agent`,
-- `trace_facts_logs`), so this crate never names a domain table. Their
-- statement triggers queue the trace ids each insert, update or delete
-- touched through `queue_trace_facts_rows`; the `trace_facts_rollup` job
-- drains the queue through `refresh_trace_facts`, which rebuilds those rows
-- and deletes the facts of a trace no ledger carries any more.
CREATE TABLE IF NOT EXISTS trace_facts (
    trace_id TEXT PRIMARY KEY,
    first_at TIMESTAMPTZ NOT NULL,
    last_at TIMESTAMPTZ NOT NULL,
    agent_name TEXT,
    status TEXT NOT NULL,
    ai_count BIGINT NOT NULL DEFAULT 0,
    mcp_count BIGINT NOT NULL DEFAULT 0,
    has_error BOOLEAN NOT NULL DEFAULT false,
    tool_names TEXT[] NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_trace_facts_first_at ON trace_facts(first_at DESC);
CREATE INDEX IF NOT EXISTS idx_trace_facts_last_at ON trace_facts(last_at);

-- Statement trigger body shared by every ledger that carries a trace id:
-- queues the distinct trace ids of the rows the statement inserted, updated
-- (old and new values, so a moved row refreshes both traces) or deleted.
CREATE OR REPLACE FUNCTION queue_trace_facts_rows()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    IF TG_OP = 'INSERT' THEN
        keys := ARRAY(SELECT DISTINCT n.trace_id::text FROM new_rows n
                      WHERE n.trace_id IS NOT NULL AND n.trace_id <> '');
    ELSIF TG_OP = 'DELETE' THEN
        keys := ARRAY(SELECT DISTINCT o.trace_id::text FROM old_rows o
                      WHERE o.trace_id IS NOT NULL AND o.trace_id <> '');
    ELSE
        keys := ARRAY(SELECT n.trace_id::text FROM new_rows n
                      WHERE n.trace_id IS NOT NULL AND n.trace_id <> ''
                      UNION
                      SELECT o.trace_id::text FROM old_rows o
                      WHERE o.trace_id IS NOT NULL AND o.trace_id <> '');
    END IF;
    IF cardinality(keys) > 0 THEN PERFORM queue_projection_backlog('trace_facts', keys); END IF;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION refresh_trace_facts(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE written BIGINT;
BEGIN
    WITH ai AS (SELECT * FROM trace_facts_ai(keys)),
    mcp AS (SELECT * FROM trace_facts_mcp(keys)),
    ag AS (SELECT * FROM trace_facts_agent(keys)),
    lg AS (SELECT * FROM trace_facts_logs(keys)),
    ids AS (
        SELECT ai.trace_id FROM ai UNION SELECT mcp.trace_id FROM mcp
        UNION SELECT ag.trace_id FROM ag UNION SELECT lg.trace_id FROM lg
    ), facts AS (
        SELECT i.trace_id,
               LEAST(ai.first_at, mcp.first_at, ag.first_at, lg.first_at) AS first_at,
               GREATEST(ai.last_at, mcp.last_at, ag.last_at, lg.last_at) AS last_at,
               ag.agent_name,
               CASE
                   WHEN COALESCE(ag.running, false) OR COALESCE(ai.running, false)
                        OR COALESCE(mcp.running, false) THEN 'running'
                   WHEN COALESCE(ag.failed, false) OR COALESCE(ai.failed, false)
                        OR COALESCE(mcp.failed, false)
                        OR (ag.trace_id IS NULL AND ai.trace_id IS NULL AND mcp.trace_id IS NULL
                            AND COALESCE(lg.has_error, false)) THEN 'failed'
                   WHEN COALESCE(ag.canceled, false) THEN 'canceled'
                   ELSE 'completed'
               END AS status,
               COALESCE(ai.row_count, 0) AS ai_count,
               COALESCE(mcp.row_count, 0) AS mcp_count,
               COALESCE(lg.has_error, false) AS has_error,
               COALESCE(mcp.tool_names, '{}'::TEXT[]) AS tool_names
        FROM ids i
        LEFT JOIN ai ON ai.trace_id = i.trace_id
        LEFT JOIN mcp ON mcp.trace_id = i.trace_id
        LEFT JOIN ag ON ag.trace_id = i.trace_id
        LEFT JOIN lg ON lg.trace_id = i.trace_id
    ), upserted AS (
        INSERT INTO trace_facts AS t (trace_id, first_at, last_at, agent_name, status,
                                      ai_count, mcp_count, has_error, tool_names)
        SELECT f.trace_id, f.first_at, f.last_at, f.agent_name, f.status,
               f.ai_count, f.mcp_count, f.has_error, f.tool_names
        FROM facts f
        ON CONFLICT (trace_id) DO UPDATE
        SET first_at = EXCLUDED.first_at, last_at = EXCLUDED.last_at,
            agent_name = EXCLUDED.agent_name, status = EXCLUDED.status,
            ai_count = EXCLUDED.ai_count, mcp_count = EXCLUDED.mcp_count,
            has_error = EXCLUDED.has_error, tool_names = EXCLUDED.tool_names
        WHERE (t.first_at, t.last_at, t.agent_name, t.status, t.ai_count, t.mcp_count,
               t.has_error, t.tool_names)
              IS DISTINCT FROM (EXCLUDED.first_at, EXCLUDED.last_at, EXCLUDED.agent_name,
               EXCLUDED.status, EXCLUDED.ai_count, EXCLUDED.mcp_count, EXCLUDED.has_error,
               EXCLUDED.tool_names)
        RETURNING 1
    ), removed AS (
        DELETE FROM trace_facts t
        WHERE t.trace_id = ANY(keys) AND NOT EXISTS (SELECT 1 FROM ids WHERE ids.trace_id = t.trace_id)
        RETURNING 1
    )
    SELECT (SELECT COUNT(*) FROM upserted) + (SELECT COUNT(*) FROM removed) INTO written;
    RETURN written;
END $$;
