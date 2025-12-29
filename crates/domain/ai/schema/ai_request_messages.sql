CREATE TABLE IF NOT EXISTS ai_request_messages (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id VARCHAR(255) NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
    content TEXT NOT NULL,
    sequence_number INTEGER NOT NULL,
    name VARCHAR(255),
    tool_call_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (request_id) REFERENCES ai_requests(id) ON DELETE CASCADE,
    UNIQUE(request_id, sequence_number)
);
CREATE INDEX IF NOT EXISTS idx_ai_request_messages_request_id ON ai_request_messages(request_id);
CREATE INDEX IF NOT EXISTS idx_ai_request_messages_role ON ai_request_messages(role);
CREATE INDEX IF NOT EXISTS idx_ai_request_messages_sequence ON ai_request_messages(request_id, sequence_number);
