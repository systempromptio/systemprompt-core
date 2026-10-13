-- How far into a replayed conversation the gateway has ingested tool results.
--
-- A client re-sends its whole history every turn. The gateway records, per
-- conversation, the index of the last message whose `tool_result` blocks it
-- handed to the artifact ingest and the `tool_use_id` of the last such block.
-- The next turn starts after that message when the same block is still at the
-- same index, so replayed history is never scanned again; any other shape
-- (compaction, an edited history) falls back to a full scan.
-- `scope_id` is the client session id, else the context id;
-- `gateway_conversation_id` is empty when the client sent none.
CREATE TABLE IF NOT EXISTS tool_result_ingest_marks (
    user_id VARCHAR(255) NOT NULL,
    scope_id VARCHAR(255) NOT NULL,
    gateway_conversation_id VARCHAR(255) NOT NULL DEFAULT '',
    last_message_index INTEGER NOT NULL,
    boundary_tool_use_id VARCHAR(255) NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, scope_id, gateway_conversation_id)
);

CREATE INDEX IF NOT EXISTS idx_tool_result_ingest_marks_updated_at ON tool_result_ingest_marks(updated_at);
