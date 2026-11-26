-- The agent of each task, as the AI rollups resolve it: one row per task
-- that names an agent. `ai_usage_rollup_hourly` and
-- `ai_context_rollup_hourly` read their `agent_name` dimension here, and
-- the `mcp` member's `tool_usage_rollup_hourly` does the same, so neither
-- names a table of the optional `agent` member. The `agent` member keeps it
-- current with statement triggers on `agent_tasks`; a composition without
-- `agent` has no rows, and every request rolls up under the empty agent.

CREATE TABLE IF NOT EXISTS ai_task_agents (
    task_id TEXT PRIMARY KEY,
    agent_name TEXT NOT NULL
);
