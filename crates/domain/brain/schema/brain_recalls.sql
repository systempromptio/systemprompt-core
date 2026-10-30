CREATE TABLE IF NOT EXISTS brain_recalls (
    id TEXT PRIMARY KEY,
    principal_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ai_request_id TEXT,
    gateway_conversation_id TEXT,
    session_id TEXT,
    context_id TEXT,
    client TEXT NOT NULL,
    project TEXT,
    injected JSONB NOT NULL,
    withheld JSONB NOT NULL,
    tokens INT NOT NULL DEFAULT 0 CHECK (tokens >= 0),
    as_of TIMESTAMPTZ NOT NULL,
    skipped TEXT,
    rendered_block TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_brain_recalls_conversation
    ON brain_recalls (gateway_conversation_id, created_at DESC)
    WHERE gateway_conversation_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_brain_recalls_principal
    ON brain_recalls (principal_user_id);
CREATE INDEX IF NOT EXISTS idx_brain_recalls_ai_request
    ON brain_recalls (ai_request_id) WHERE ai_request_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_brain_recalls_created
    ON brain_recalls (created_at);
