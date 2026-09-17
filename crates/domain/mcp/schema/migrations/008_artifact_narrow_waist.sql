-- Make every tool result a first-class, linked artifact.
--
-- Artifacts were persisted only for in-process tools, keyed by artifact id,
-- with trace and session buried in JSON and no key back to the client's
-- `tool_use_id`. Executions had no notion of where they were observed. This
-- migration promotes the correlation keys to columns, records the vantage
-- point (`source`) and join quality (`correlation`) on both rows, moves
-- artifact bodies into the content-addressed `artifact_payloads` store, and
-- gives every artifact a real execution to belong to.

-- Executions: where seen, how joined, what body.
ALTER TABLE mcp_tool_executions ADD COLUMN IF NOT EXISTS source VARCHAR(32) NOT NULL DEFAULT 'in_process';
ALTER TABLE mcp_tool_executions ADD COLUMN IF NOT EXISTS correlation VARCHAR(16) NOT NULL DEFAULT 'exact';
ALTER TABLE mcp_tool_executions ADD COLUMN IF NOT EXISTS payload_sha256 CHAR(64);
ALTER TABLE mcp_tool_executions DROP CONSTRAINT IF EXISTS mcp_tool_executions_source_check;
ALTER TABLE mcp_tool_executions ADD CONSTRAINT mcp_tool_executions_source_check
    CHECK (source IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode'));
ALTER TABLE mcp_tool_executions DROP CONSTRAINT IF EXISTS mcp_tool_executions_correlation_check;
ALTER TABLE mcp_tool_executions ADD CONSTRAINT mcp_tool_executions_correlation_check
    CHECK (correlation IN ('exact', 'inferred'));
-- Historical proxy-tapped rows are recognisable by their request method.
UPDATE mcp_tool_executions SET source = 'proxy' WHERE request_method = 'mcp' AND source = 'in_process';

-- Payload store must exist before artifacts can point at it.
CREATE TABLE IF NOT EXISTS artifact_payloads (
    sha256 CHAR(64) PRIMARY KEY,
    byte_len INTEGER NOT NULL,
    body JSONB NOT NULL,
    ref_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Artifacts: promoted keys and classification.
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS session_id VARCHAR(255);
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS trace_id VARCHAR(255);
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS ai_tool_call_id VARCHAR(255);
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS tool_name VARCHAR(255);
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS source VARCHAR(32) NOT NULL DEFAULT 'in_process';
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS last_seen_source VARCHAR(32);
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS payload_sha256 CHAR(64);
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS payload_bytes INTEGER;
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS is_structured BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS has_ui_resource BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS is_error BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE mcp_artifacts ADD COLUMN IF NOT EXISTS secret_redactions INTEGER NOT NULL DEFAULT 0;
ALTER TABLE mcp_artifacts DROP CONSTRAINT IF EXISTS mcp_artifacts_source_check;
ALTER TABLE mcp_artifacts ADD CONSTRAINT mcp_artifacts_source_check
    CHECK (source IN ('in_process', 'proxy', 'gateway', 'hook_claude_code', 'hook_opencode'));

-- Backfill from the execution metadata every in-process artifact carried.
UPDATE mcp_artifacts
SET session_id = COALESCE(session_id, NULLIF(metadata->>'session_id', 'unset')),
    trace_id = COALESCE(trace_id, NULLIF(metadata->>'trace_id', 'unset')),
    tool_name = COALESCE(tool_name, metadata->>'tool_name'),
    last_seen_source = COALESCE(last_seen_source, source),
    is_structured = TRUE
WHERE payload_sha256 IS NULL;

-- Move the artifact body into the content-addressed store.
INSERT INTO artifact_payloads (sha256, byte_len, body, ref_count)
SELECT encode(sha256(convert_to((data->'artifact')::text, 'UTF8')), 'hex'),
       octet_length((data->'artifact')::text),
       data->'artifact',
       count(*)
FROM mcp_artifacts
WHERE payload_sha256 IS NULL AND data ? 'artifact'
GROUP BY 1, 2, 3
ON CONFLICT (sha256) DO UPDATE SET ref_count = artifact_payloads.ref_count + EXCLUDED.ref_count;

UPDATE mcp_artifacts
SET payload_sha256 = encode(sha256(convert_to((data->'artifact')::text, 'UTF8')), 'hex'),
    payload_bytes = octet_length((data->'artifact')::text)
WHERE payload_sha256 IS NULL AND data ? 'artifact';

-- Every artifact belongs to an execution: give orphans the execution their
-- own metadata describes rather than dropping them.
INSERT INTO mcp_tool_executions (
    mcp_execution_id, tool_name, server_name, started_at, completed_at, execution_time_ms,
    input, output, status, user_id, session_id, context_id, trace_id, request_method,
    request_source, source, correlation, created_at
)
SELECT a.mcp_execution_id,
       COALESCE(a.tool_name, a.artifact_type),
       a.server_name,
       a.created_at, a.created_at, 0,
       '{}', NULL, 'success',
       COALESCE(a.user_id, NULLIF(a.metadata->>'user_id', 'unset'), 'unknown'),
       a.session_id, a.context_id, a.trace_id,
       'mcp', a.server_name, a.source, 'exact', a.created_at
FROM mcp_artifacts a
LEFT JOIN mcp_tool_executions e ON e.mcp_execution_id = a.mcp_execution_id
WHERE e.mcp_execution_id IS NULL
ON CONFLICT DO NOTHING;

-- One artifact per execution: an execution that somehow carried two keeps the
-- newest, which is the one every reader already resolved to.
DELETE FROM mcp_artifacts a
USING mcp_artifacts b
WHERE a.mcp_execution_id = b.mcp_execution_id
  AND a.created_at < b.created_at;

CREATE UNIQUE INDEX IF NOT EXISTS idx_mcp_artifacts_execution ON mcp_artifacts(mcp_execution_id);
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_session_created ON mcp_artifacts(session_id, created_at DESC) WHERE session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_trace ON mcp_artifacts(trace_id) WHERE trace_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_ai_tool_call ON mcp_artifacts(ai_tool_call_id) WHERE ai_tool_call_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_structured ON mcp_artifacts(created_at DESC) WHERE is_structured;
CREATE INDEX IF NOT EXISTS idx_mcp_artifacts_payload ON mcp_artifacts(payload_sha256) WHERE payload_sha256 IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_tool_executions_session_started ON mcp_tool_executions(session_id, started_at DESC) WHERE session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_tool_executions_source ON mcp_tool_executions(source, started_at DESC);
