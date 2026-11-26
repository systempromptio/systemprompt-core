-- The AI schema names no table of an optional member. The tool-call intent's
-- `mcp_execution_id` is a soft link: the `mcp` member may be absent, so the
-- foreign key to `mcp_tool_executions` is dropped (a dangling id joins no
-- execution, as NULL does). The rollups resolve a task's agent through
-- `ai_task_agents`, which the declarative schema creates before migrations
-- run and the `agent` member fills.
ALTER TABLE ai_request_tool_calls
    DROP CONSTRAINT IF EXISTS ai_request_tool_calls_mcp_execution_id_fkey;
