-- `ai_request_tool_calls` records whether each tool call's arguments satisfy
-- the input schema the tool was offered with, and the JSON pointer of the
-- first failing instance; `tool_call_ledger` exposes both. Existing rows keep
-- NULL: nothing is backfilled.
--
-- Why: lock footprint. The ALTER takes ACCESS EXCLUSIVE for a catalog-only
-- change (nullable columns, no default, no rewrite). The CHECK is added NOT
-- VALID and validated separately under SHARE UPDATE EXCLUSIVE, which does not
-- block writers.
ALTER TABLE ai_request_tool_calls
    ADD COLUMN IF NOT EXISTS args_validity TEXT,
    ADD COLUMN IF NOT EXISTS args_error_path TEXT;

ALTER TABLE ai_request_tool_calls DROP CONSTRAINT IF EXISTS ai_request_tool_calls_args_validity_check;
ALTER TABLE ai_request_tool_calls
    ADD CONSTRAINT ai_request_tool_calls_args_validity_check
    CHECK (args_validity IN ('valid', 'invalid', 'unknown_tool', 'unparseable', 'schema_unusable')) NOT VALID;
ALTER TABLE ai_request_tool_calls VALIDATE CONSTRAINT ai_request_tool_calls_args_validity_check;

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
    CASE WHEN e.mcp_execution_id IS NULL THEN FALSE ELSE e.is_builtin END AS is_builtin,
    e.input_summary,
    e.artifact_kind,
    i.args_validity,
    i.args_error_path
FROM ai_request_tool_calls i
FULL OUTER JOIN mcp_tool_executions e
    ON e.ai_tool_call_id = i.ai_tool_call_id
LEFT JOIN mcp_artifacts a
    ON a.mcp_execution_id = e.mcp_execution_id
LEFT JOIN ai_requests r
    ON r.id = i.request_id;
