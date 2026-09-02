CREATE TABLE IF NOT EXISTS ai_gateway_thought_signatures (
    conversation_id TEXT NOT NULL,
    tool_use_id TEXT NOT NULL,
    signature TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (conversation_id, tool_use_id)
);

CREATE INDEX IF NOT EXISTS idx_ai_gateway_thought_signatures_expires_at
    ON ai_gateway_thought_signatures(expires_at);
