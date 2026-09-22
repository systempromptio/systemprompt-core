-- One execution per call: fold each client-hook attestation into the
-- server-observed execution it duplicates.
--
-- The proxy tap records a call as it runs (`source = 'proxy'`), and the
-- client's PostToolUse hook reported the same call again moments later as
-- its own row (`source = 'hook_%'`, no measured duration) because neither
-- shared a key with the other. The ingest now pairs the two by user, server,
-- tool and time; this rung applies the same rule to history. For every hook
-- row that carries the client's `tool_use_id`, the newest unattested
-- server-observed execution of that tool by that user in the 30 s before it
-- takes the key (`correlation = 'inferred'`), the gateway intents that
-- pointed at the hook row are re-pointed, the hook's artifact moves across
-- when the server row has none (an execution owns one artifact; otherwise
-- the hook's copy goes with its row), and the hook row goes.
-- `plugin_usage_events.mcp_execution_id` belongs to the console and is
-- re-pointed by its owner through `tool_use_id`. Guarded by
-- `ai_tool_call_id IS NULL` on the target, so a second run pairs nothing.
-- Hook rows that pair with nothing (in-process calls a hook alone saw) lose
-- their invented 0 ms duration so latency figures ignore them.
CREATE TEMP TABLE hook_pairs ON COMMIT DROP AS
SELECT DISTINCT ON (s.mcp_execution_id)
    h.mcp_execution_id AS hook_id,
    s.mcp_execution_id AS server_id,
    h.ai_tool_call_id
FROM mcp_tool_executions h
JOIN LATERAL (
    SELECT e.mcp_execution_id, e.started_at
    FROM mcp_tool_executions e
    WHERE e.source IN ('in_process', 'proxy')
      AND e.ai_tool_call_id IS NULL
      AND e.user_id = h.user_id
      AND e.server_name = h.server_name
      AND e.tool_name = h.tool_name
      AND e.started_at BETWEEN h.started_at - INTERVAL '30 seconds' AND h.started_at
    ORDER BY e.started_at DESC
    LIMIT 1
) s ON TRUE
WHERE h.source LIKE 'hook_%'
  AND h.ai_tool_call_id IS NOT NULL
ORDER BY s.mcp_execution_id, h.started_at ASC;

UPDATE mcp_tool_executions e
SET ai_tool_call_id = NULL
FROM hook_pairs p
WHERE e.mcp_execution_id = p.hook_id;

UPDATE mcp_tool_executions e
SET ai_tool_call_id = p.ai_tool_call_id,
    correlation = 'inferred'
FROM hook_pairs p
WHERE e.mcp_execution_id = p.server_id;

UPDATE mcp_artifacts a
SET mcp_execution_id = p.server_id
FROM hook_pairs p
WHERE a.mcp_execution_id = p.hook_id
  AND NOT EXISTS (
      SELECT 1 FROM mcp_artifacts s WHERE s.mcp_execution_id = p.server_id
  );

UPDATE ai_request_tool_calls c
SET mcp_execution_id = p.server_id
FROM hook_pairs p
WHERE c.mcp_execution_id = p.hook_id;

DELETE FROM mcp_tool_executions e
USING hook_pairs p
WHERE e.mcp_execution_id = p.hook_id;

UPDATE mcp_tool_executions
SET execution_time_ms = NULL
WHERE source LIKE 'hook_%'
  AND execution_time_ms IS NOT NULL;
