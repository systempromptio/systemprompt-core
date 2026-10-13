-- Trace facts (schema/trace_facts.sql): one row per trace, read by the trace
-- list instead of unioning four ledgers per load. The table is the base
-- schema's text; each ledger's owner queues its own trace ids in its own
-- migration (ai 039, mcp 020, agent 015, logging 010) and the
-- `trace_facts_rollup` job drains them through `refresh_trace_facts`.
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
