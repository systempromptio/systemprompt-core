-- Record which surface invoked the tool.
--
-- Executions carried only the user; the actor kind that ran on that user's
-- behalf (user, agent, mcp server, job) lived nowhere, so an execution could
-- not be attributed beyond the account. Nullable and unconstrained: a CHECK
-- that lags the ActorKind enum is what forced governance_decisions 005.
-- Historical rows stay NULL rather than being backfilled with a guess.

ALTER TABLE mcp_tool_executions ADD COLUMN IF NOT EXISTS actor_kind TEXT;
ALTER TABLE mcp_tool_executions ADD COLUMN IF NOT EXISTS actor_id TEXT;

CREATE INDEX IF NOT EXISTS idx_mcp_tool_executions_actor ON mcp_tool_executions(actor_kind, actor_id);
