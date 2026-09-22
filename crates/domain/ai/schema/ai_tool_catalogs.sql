-- One row per distinct tool list, keyed by the digest of its canonical JSONB
-- text. A harness sends the same catalogue on every request of a session, so
-- payload rows reference the list instead of carrying it: a production
-- instance held 4,392 copies of 152 distinct lists (433 MB) before this.
CREATE TABLE IF NOT EXISTS ai_tool_catalogs (
    sha256 TEXT PRIMARY KEY CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    tools JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
