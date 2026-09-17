-- Scanner findings raised against a tool-result artifact at ingestion.
--
-- Secret and safety scanners run before an artifact row exists; each finding
-- is recorded here against the artifact, so a stored body is always one that
-- has been scanned and, where a secret was found, redacted. This is the
-- artifact-side counterpart of `ai_safety_findings`, which is keyed by AI
-- request; the two are kept apart so each row has exactly one subject.
CREATE TABLE IF NOT EXISTS mcp_artifact_findings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    artifact_id VARCHAR(255) NOT NULL,
    phase VARCHAR(32) NOT NULL,
    severity VARCHAR(16) NOT NULL,
    category VARCHAR(64) NOT NULL,
    scanner VARCHAR(64) NOT NULL,
    path TEXT,
    excerpt TEXT,
    redacted BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (artifact_id) REFERENCES mcp_artifacts(artifact_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_mcp_artifact_findings_artifact ON mcp_artifact_findings(artifact_id);
CREATE INDEX IF NOT EXISTS idx_mcp_artifact_findings_category ON mcp_artifact_findings(category, created_at DESC);
