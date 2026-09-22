-- The tool-call ledger: one row per tool call from any client, joining the
-- three entities that each mean one thing.
--
--   intent     ai_request_tool_calls   the model emitted a tool_use (gateway, all clients)
--   execution  mcp_tool_executions     the call ran (in-process, proxy, or client hook)
--   result     mcp_artifacts           the typed artifact the execution produced
--
-- They join on `ai_tool_call_id`, the client's `tool_use_id`. A FULL OUTER
-- join keeps intent without execution (a call the client never reported) and
-- execution without intent (a call that bypassed the gateway), and `state`
-- names which of those a row is. Every dashboard tool count reads this view.
-- `mcp_artifacts.mcp_execution_id` is unique, so one execution is exactly
-- one ledger row. `is_builtin` marks a host-native tool (Bash, Read, …) a
-- hook reported with no MCP server behind it, recorded under the vantage
-- point's own name; consumers that mean "MCP tools" exclude it.
CREATE OR REPLACE VIEW tool_call_ledger AS
SELECT
    COALESCE(i.ai_tool_call_id, e.ai_tool_call_id) AS ai_tool_call_id,
    i.id AS intent_id,
    i.request_id,
    e.mcp_execution_id,
    a.artifact_id,
    COALESCE(r.user_id, e.user_id, a.user_id) AS user_id,
    COALESCE(r.session_id, e.session_id, a.session_id) AS session_id,
    COALESCE(r.context_id, e.context_id, a.context_id) AS context_id,
    COALESCE(r.trace_id, e.trace_id, a.trace_id) AS trace_id,
    r.client_kind,
    COALESCE(i.tool_name, e.tool_name, a.tool_name) AS tool_name,
    COALESCE(e.server_name, a.server_name) AS server_name,
    i.created_at AS intended_at,
    e.started_at AS executed_at,
    e.completed_at,
    e.execution_time_ms,
    e.status AS execution_status,
    e.error_message,
    e.source,
    e.correlation,
    a.artifact_type,
    a.title AS artifact_title,
    COALESCE(a.is_structured, FALSE) AS is_structured,
    COALESCE(a.has_ui_resource, FALSE) AS has_ui_resource,
    COALESCE(a.is_error, FALSE) AS is_error,
    a.payload_bytes,
    a.secret_redactions,
    CASE
        WHEN i.id IS NULL THEN 'unattested'
        WHEN e.mcp_execution_id IS NULL THEN 'intended'
        ELSE 'executed'
    END AS state,
    COALESCE(e.started_at, i.created_at) AS occurred_at,
    (e.mcp_execution_id IS NOT NULL AND e.server_name = e.source) AS is_builtin
FROM ai_request_tool_calls i
FULL OUTER JOIN mcp_tool_executions e
    ON e.ai_tool_call_id = i.ai_tool_call_id
LEFT JOIN mcp_artifacts a
    ON a.mcp_execution_id = e.mcp_execution_id
LEFT JOIN ai_requests r
    ON r.id = i.request_id;
