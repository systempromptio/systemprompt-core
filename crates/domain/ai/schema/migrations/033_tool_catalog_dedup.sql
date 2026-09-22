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

INSERT INTO ai_tool_catalogs (sha256, tools)
SELECT DISTINCT ON (encode(sha256(convert_to(t.tools::text, 'UTF8')), 'hex'))
       encode(sha256(convert_to(t.tools::text, 'UTF8')), 'hex'), t.tools
FROM (
    SELECT offered_tools AS tools FROM ai_request_payloads WHERE offered_tools IS NOT NULL
    UNION ALL
    SELECT prepared_tools FROM ai_request_payloads WHERE prepared_tools IS NOT NULL
) t
ON CONFLICT (sha256) DO NOTHING;

UPDATE ai_request_payloads
SET offered_tools_sha256 = encode(sha256(convert_to(offered_tools::text, 'UTF8')), 'hex')
WHERE offered_tools IS NOT NULL AND offered_tools_sha256 IS NULL;

UPDATE ai_request_payloads
SET prepared_tools_sha256 = encode(sha256(convert_to(prepared_tools::text, 'UTF8')), 'hex')
WHERE prepared_tools IS NOT NULL AND prepared_tools_sha256 IS NULL;

ALTER TABLE ai_request_payloads
    DROP COLUMN IF EXISTS offered_tools CASCADE,
    DROP COLUMN IF EXISTS prepared_tools CASCADE;
