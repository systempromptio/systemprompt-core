-- Key a pre-tool decision to the call it governed.
--
-- A client hook names the call by its `tool_use_id`; the decision row did not
-- keep it, so joining a decision to the execution and artifact of the same
-- call fell back to a time window. It is its own column now, and nullable:
-- enforcement sites without a client call id (gateway prompts, server-side
-- MCP) leave it NULL.

ALTER TABLE governance_decisions ADD COLUMN IF NOT EXISTS tool_use_id TEXT;
CREATE INDEX IF NOT EXISTS idx_governance_decisions_tool_use_id ON governance_decisions(tool_use_id) WHERE tool_use_id IS NOT NULL;
