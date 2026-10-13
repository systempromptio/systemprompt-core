-- Tool row facts: what a tool call was about, whether it was a host-native
-- tool, and which kind of artifact (if any) it left, stored on the row so no
-- reader re-classifies the ledger on every load (store, don't compute).
--
--   mcp_tool_executions.input_summary  the file path, command, pattern, query,
--                                      path, url, skill or name of the input,
--                                      else its first key, else a clip
--   mcp_tool_executions.is_builtin     a host-native tool (Bash, Read, …) a
--                                      vantage point reported under its own
--                                      name (`server_name = source`), unless
--                                      the tool is an `mcp__` one
--   mcp_tool_executions.artifact_kind  the call's artifact kind
--   mcp_artifacts.artifact_kind        the same kind, on the artifact row
--
-- The artifact rule: a file the assistant edited, wrote or read is 'file'; a
-- builtin is never an artifact; an MCP Apps resource is 'ui'; a typed card is
-- 'card'; a retained structured body is 'body'; anything else is a plain call
-- (NULL). NULL `is_builtin` means the row has not been projected yet.
--
-- `refresh_tool_row_facts(keys)` takes execution ids and stamps both tables.
-- Statement triggers call it for the rows each statement inserted, and for
-- rows whose inputs to the rule changed (a later payload hash, an artifact
-- saved again); its own writes touch only fact columns, so the update
-- triggers find no changed keys and stop (a statement trigger fires even for
-- zero rows, hence the empty-keys guard). Migration 019 queues the executions that predate the
-- columns and the `tool_row_facts_rollup` job drains them.

CREATE OR REPLACE FUNCTION mcp_tool_input_summary(input text)
RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE body jsonb;
BEGIN
    IF input IS NULL OR input = '' THEN RETURN NULL; END IF;
    IF left(ltrim(input), 1) <> '{' THEN RETURN left(input, 160); END IF;
    body := input::jsonb;
    RETURN COALESCE(
        NULLIF(body->>'file_path', ''), NULLIF(body->>'command', ''),
        NULLIF(body->>'pattern', ''), NULLIF(body->>'query', ''),
        NULLIF(body->>'path', ''), NULLIF(body->>'url', ''),
        NULLIF(body->>'skill', ''), NULLIF(body->>'name', ''),
        (SELECT k FROM jsonb_object_keys(body) k LIMIT 1));
EXCEPTION WHEN others THEN
    RETURN left(input, 160);
END $$;

CREATE OR REPLACE FUNCTION mcp_tool_artifact_kind(tool_name text, artifact_type text, has_ui boolean,
    is_structured boolean, payload_sha256 text, input text, is_builtin boolean)
RETURNS text LANGUAGE sql IMMUTABLE AS $$
SELECT CASE
    WHEN tool_name IN ('Edit', 'Write', 'MultiEdit', 'NotebookEdit', 'Read')
         AND input IS NOT NULL AND position('"file_path"' IN input) > 0 THEN 'file'
    WHEN COALESCE(is_builtin, false) THEN NULL
    WHEN COALESCE(has_ui, false) THEN 'ui'
    WHEN artifact_type IS NOT NULL AND artifact_type <> 'tool_result' THEN 'card'
    WHEN payload_sha256 IS NOT NULL AND COALESCE(is_structured, false) THEN 'body'
    ELSE NULL END
$$;

CREATE OR REPLACE FUNCTION refresh_tool_row_facts(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE written BIGINT;
BEGIN
    WITH rows AS (
        SELECT e.mcp_execution_id, a.artifact_id, e.tool_name, e.input,
               a.artifact_type, a.has_ui_resource, a.is_structured,
               COALESCE(a.payload_sha256, e.payload_sha256) AS payload_sha256,
               (e.server_name = e.source AND e.tool_name NOT LIKE 'mcp\_\_%') AS is_builtin
        FROM mcp_tool_executions e
        LEFT JOIN mcp_artifacts a ON a.mcp_execution_id = e.mcp_execution_id
        WHERE e.mcp_execution_id = ANY(keys)
    ), facts AS (
        SELECT r.mcp_execution_id, r.artifact_id, r.is_builtin,
               mcp_tool_input_summary(r.input) AS input_summary,
               mcp_tool_artifact_kind(r.tool_name, r.artifact_type, r.has_ui_resource,
                   r.is_structured, r.payload_sha256, r.input, r.is_builtin) AS artifact_kind
        FROM rows r
    ), executions AS (
        UPDATE mcp_tool_executions e
        SET input_summary = f.input_summary, is_builtin = f.is_builtin,
            artifact_kind = f.artifact_kind
        FROM facts f
        WHERE e.mcp_execution_id = f.mcp_execution_id
          AND (e.input_summary, e.is_builtin, e.artifact_kind)
              IS DISTINCT FROM (f.input_summary, f.is_builtin, f.artifact_kind)
        RETURNING 1
    ), artifacts AS (
        UPDATE mcp_artifacts a
        SET artifact_kind = f.artifact_kind
        FROM facts f
        WHERE a.artifact_id = f.artifact_id
          AND a.artifact_kind IS DISTINCT FROM f.artifact_kind
        RETURNING 1
    )
    SELECT (SELECT COUNT(*) FROM executions) + (SELECT COUNT(*) FROM artifacts) INTO written;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION stamp_tool_row_facts_inserted()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    keys := ARRAY(SELECT n.mcp_execution_id::text FROM new_rows n);
    IF cardinality(keys) > 0 THEN PERFORM refresh_tool_row_facts(keys); END IF;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION stamp_tool_row_facts_execution_updated()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    keys := ARRAY(
        SELECT n.mcp_execution_id FROM new_rows n
        JOIN old_rows o ON o.mcp_execution_id = n.mcp_execution_id
        WHERE (n.tool_name, n.server_name, n.source, n.payload_sha256)
              IS DISTINCT FROM (o.tool_name, o.server_name, o.source, o.payload_sha256));
    IF cardinality(keys) > 0 THEN PERFORM refresh_tool_row_facts(keys); END IF;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION stamp_tool_row_facts_artifact_updated()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    keys := ARRAY(
        SELECT n.mcp_execution_id::text FROM new_rows n
        JOIN old_rows o ON o.id = n.id
        WHERE (n.mcp_execution_id, n.artifact_type, n.has_ui_resource, n.is_structured, n.payload_sha256)
              IS DISTINCT FROM (o.mcp_execution_id, o.artifact_type, o.has_ui_resource, o.is_structured,
                                o.payload_sha256));
    IF cardinality(keys) > 0 THEN PERFORM refresh_tool_row_facts(keys); END IF;
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS tool_row_facts_execution_insert ON mcp_tool_executions;
CREATE TRIGGER tool_row_facts_execution_insert
    AFTER INSERT ON mcp_tool_executions
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stamp_tool_row_facts_inserted();

DROP TRIGGER IF EXISTS tool_row_facts_execution_update ON mcp_tool_executions;
CREATE TRIGGER tool_row_facts_execution_update
    AFTER UPDATE ON mcp_tool_executions
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stamp_tool_row_facts_execution_updated();

DROP TRIGGER IF EXISTS tool_row_facts_artifact_insert ON mcp_artifacts;
CREATE TRIGGER tool_row_facts_artifact_insert
    AFTER INSERT ON mcp_artifacts
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stamp_tool_row_facts_inserted();

DROP TRIGGER IF EXISTS tool_row_facts_artifact_update ON mcp_artifacts;
CREATE TRIGGER tool_row_facts_artifact_update
    AFTER UPDATE ON mcp_artifacts
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stamp_tool_row_facts_artifact_updated();
