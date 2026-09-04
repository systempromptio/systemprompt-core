CREATE TABLE IF NOT EXISTS ai_safety_findings (
    id TEXT PRIMARY KEY,
    ai_request_id TEXT NOT NULL,
    phase VARCHAR(32) NOT NULL,
    severity VARCHAR(16) NOT NULL,
    category VARCHAR(64) NOT NULL,
    scanner VARCHAR(64) NOT NULL,
    excerpt TEXT,
    -- Whether this finding actually refused the call. Under `safety.mode:
    -- warn` a finding in a block category is recorded and the call proceeds,
    -- so "matched a block category" and "blocked" stop being the same fact and
    -- the report needs both.
    blocked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (ai_request_id) REFERENCES ai_requests(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_ai_safety_findings_request ON ai_safety_findings(ai_request_id);
CREATE INDEX IF NOT EXISTS idx_ai_safety_findings_severity ON ai_safety_findings(severity);
CREATE INDEX IF NOT EXISTS idx_ai_safety_findings_category ON ai_safety_findings(category);
CREATE INDEX IF NOT EXISTS idx_ai_safety_findings_created_at ON ai_safety_findings(created_at);
