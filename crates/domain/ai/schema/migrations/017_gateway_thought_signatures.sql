-- Gemini thought signatures, keyed by gateway conversation and tool_use id.
--
-- Signatures were cached in a process-local map, so a replay routed to a
-- different replica (or a restarted process) could not re-sign the tool_use
-- block and Gemini rejected the turn. Persisting them makes hydration
-- replica-independent.

CREATE TABLE IF NOT EXISTS ai_gateway_thought_signatures (
    conversation_id TEXT NOT NULL,
    tool_use_id TEXT NOT NULL,
    signature TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (conversation_id, tool_use_id)
);

CREATE INDEX IF NOT EXISTS idx_ai_gateway_thought_signatures_expires_at
    ON ai_gateway_thought_signatures(expires_at);
