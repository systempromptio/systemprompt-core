-- Enforce the two foreign keys `mcp_artifacts.sql` declares on databases
-- that created the table before they existed. Rows the keys would reject are
-- reconciled first: an artifact without an execution is dropped, and a
-- payload digest without a body is cleared.
DELETE FROM mcp_artifacts a
WHERE NOT EXISTS (SELECT 1 FROM mcp_tool_executions e WHERE e.mcp_execution_id = a.mcp_execution_id);

UPDATE mcp_artifacts a
SET payload_sha256 = NULL
WHERE a.payload_sha256 IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM artifact_payloads p WHERE p.sha256 = a.payload_sha256);

ALTER TABLE mcp_artifacts DROP CONSTRAINT IF EXISTS mcp_artifacts_mcp_execution_id_fkey;
ALTER TABLE mcp_artifacts ADD CONSTRAINT mcp_artifacts_mcp_execution_id_fkey
    FOREIGN KEY (mcp_execution_id) REFERENCES mcp_tool_executions(mcp_execution_id) ON DELETE CASCADE;

ALTER TABLE mcp_artifacts DROP CONSTRAINT IF EXISTS mcp_artifacts_payload_sha256_fkey;
ALTER TABLE mcp_artifacts ADD CONSTRAINT mcp_artifacts_payload_sha256_fkey
    FOREIGN KEY (payload_sha256) REFERENCES artifact_payloads(sha256) ON DELETE SET NULL;
