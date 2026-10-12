-- Offered-tools flag: whether a request offered the model client tools,
-- stored on the request row so no reader joins the payload ledger to ask
-- (store, don't compute).
--
--   ai_requests.has_offered_tools  TRUE when the request's payload names an
--                                  offered-tools catalog
--                                  (`ai_request_payloads.offered_tools_sha256`)
--
-- The stamp is written by the request's owner in the transaction that writes
-- the row: the gateway admission knows whether the body carried tools before
-- it inserts, and the offered-tools upsert sets the flag in the same
-- statement that links the catalog, for writers that store the tools after
-- the row. NULL means the row predates the column and has not been projected.
--
-- `refresh_ai_request_tool_flag(keys)` takes request ids and stamps them from
-- their payload row; it writes only rows whose flag differs. Migration 041
-- queues the requests that predate the column and the
-- `ai_request_tool_flag_rollup` job drains them.

CREATE OR REPLACE FUNCTION refresh_ai_request_tool_flag(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE written BIGINT;
BEGIN
    WITH flags AS (
        SELECT r.id, EXISTS (
            SELECT 1 FROM ai_request_payloads p
            WHERE p.ai_request_id = r.id AND p.offered_tools_sha256 IS NOT NULL) AS has_offered_tools
        FROM ai_requests r
        WHERE r.id = ANY(keys)
    ), stamped AS (
        UPDATE ai_requests r SET has_offered_tools = f.has_offered_tools
        FROM flags f
        WHERE r.id = f.id AND r.has_offered_tools IS DISTINCT FROM f.has_offered_tools
        RETURNING 1
    )
    SELECT COUNT(*) INTO written FROM stamped;
    RETURN written;
END $$;
