-- Tool row facts (schema/tool_row_facts.sql): the input summary, the builtin
-- flag and the artifact kind are stored on the row instead of re-derived by
-- every reader. The columns are the base schema's text; the executions that
-- predate them are queued for `refresh_tool_row_facts`, which the
-- `tool_row_facts_rollup` job drains. One key per execution fills its
-- artifact too.
ALTER TABLE mcp_tool_executions ADD COLUMN IF NOT EXISTS input_summary TEXT;
ALTER TABLE mcp_tool_executions ADD COLUMN IF NOT EXISTS is_builtin BOOLEAN;
ALTER TABLE mcp_tool_executions ADD COLUMN IF NOT EXISTS artifact_kind TEXT;
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS artifact_kind TEXT;

INSERT INTO projection_backlog (projection, key)
SELECT 'tool_row_facts', mcp_execution_id FROM mcp_tool_executions WHERE is_builtin IS NULL
ON CONFLICT DO NOTHING;
