-- Migration 008 relabelled every historical in-process execution as `proxy`
-- by request method alone. The in-process executor always persisted an
-- artifact and the proxy tap never did, so the artifact row is the
-- discriminator: executions that own one stamped `in_process` (008 gave every
-- pre-existing artifact that source) go back to `in_process`; an artifact the
-- 0.55 proxy tap ingested carries `proxy` and its execution stays. 008 keeps
-- the text it was applied with, so a database that already ran it upgrades
-- through this repair instead of refusing on checksum drift.
UPDATE mcp_tool_executions e
SET source = 'in_process'
WHERE e.request_method = 'mcp'
  AND e.source = 'proxy'
  AND EXISTS (
      SELECT 1 FROM mcp_artifacts a
      WHERE a.mcp_execution_id = e.mcp_execution_id
        AND a.source = 'in_process'
  );
