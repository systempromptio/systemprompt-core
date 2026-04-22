CREATE TABLE IF NOT EXISTS ai_request_payloads (
    ai_request_id TEXT PRIMARY KEY,
    request_body JSONB,
    response_body JSONB,
    request_excerpt TEXT,
    response_excerpt TEXT,
    request_truncated BOOLEAN NOT NULL DEFAULT FALSE,
    response_truncated BOOLEAN NOT NULL DEFAULT FALSE,
    request_bytes INTEGER,
    response_bytes INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (ai_request_id) REFERENCES ai_requests(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_ai_request_payloads_created_at ON ai_request_payloads(created_at);
