-- @supersedes-checksum: beac575d8fec4552
-- @cost: rows=3199 measured=4.6s triggers=live
-- Store each distinct tool list once and reference it from the payload row.
--
-- `offered_tools` (the client's `tools` array) and `prepared_tools` (the
-- array the gateway sent upstream) were copied in full onto every payload
-- row. A harness offers the same catalogue on every turn of a session, so a
-- production instance held 4,392 copies of 152 distinct lists — 433 MB that
-- deduplicates to 19 MB. Both columns become digests into `ai_tool_catalogs`;
-- the digest is of the canonical JSONB text, computed in the database so the
-- backfill and the runtime writer agree.
--
-- Dependent views outside this extension (the astound `conversation_requests`
-- view tests `offered_tools IS NOT NULL`) are dropped by the CASCADE and
-- recreated by their own extension's next migration against the new column.
CREATE TABLE IF NOT EXISTS ai_tool_catalogs (
    sha256 TEXT PRIMARY KEY CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    tools JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE ai_request_payloads
    ADD COLUMN IF NOT EXISTS offered_tools_sha256 TEXT REFERENCES ai_tool_catalogs(sha256),
    ADD COLUMN IF NOT EXISTS prepared_tools_sha256 TEXT REFERENCES ai_tool_catalogs(sha256);

-- The backfill names columns this migration's last statement drops, so a
-- re-run on a database that already reached the end state fails to plan —
-- Postgres resolves column names before any WHERE guard can skip the
-- statement. Running it through EXECUTE keeps the three statements out of
-- the planner until the guard has confirmed the columns are still there,
-- which is what makes the migration re-runnable after a partial apply.
DO $backfill$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = current_schema()
          AND table_name = 'ai_request_payloads'
          AND column_name = 'offered_tools'
    ) THEN
        RETURN;
    END IF;

    EXECUTE $sql$
        INSERT INTO ai_tool_catalogs (sha256, tools)
        SELECT DISTINCT ON (encode(sha256(convert_to(t.tools::text, 'UTF8')), 'hex'))
               encode(sha256(convert_to(t.tools::text, 'UTF8')), 'hex'), t.tools
        FROM (
            SELECT offered_tools AS tools FROM ai_request_payloads WHERE offered_tools IS NOT NULL
            UNION ALL
            SELECT prepared_tools FROM ai_request_payloads WHERE prepared_tools IS NOT NULL
        ) t
        ON CONFLICT (sha256) DO NOTHING
    $sql$;

    EXECUTE $sql$
        UPDATE ai_request_payloads
        SET offered_tools_sha256 = encode(sha256(convert_to(offered_tools::text, 'UTF8')), 'hex')
        WHERE offered_tools IS NOT NULL AND offered_tools_sha256 IS NULL
    $sql$;

    EXECUTE $sql$
        UPDATE ai_request_payloads
        SET prepared_tools_sha256 = encode(sha256(convert_to(prepared_tools::text, 'UTF8')), 'hex')
        WHERE prepared_tools IS NOT NULL AND prepared_tools_sha256 IS NULL
    $sql$;
END
$backfill$;

ALTER TABLE ai_request_payloads
    DROP COLUMN IF EXISTS offered_tools CASCADE,
    DROP COLUMN IF EXISTS prepared_tools CASCADE;
