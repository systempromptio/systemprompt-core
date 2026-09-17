-- Content-addressed bodies of tool-result artifacts.
--
-- A tool result is stored once per distinct content, keyed by the SHA-256 of
-- its canonical JSON. A gateway history that replays the same result on every
-- turn, or a client that reads the same file ten times, costs one indexed hit
-- and no bytes. `ref_count` is the number of artifacts pointing at the body;
-- retention deletes the body when it reaches zero.
CREATE TABLE IF NOT EXISTS artifact_payloads (
    sha256 CHAR(64) PRIMARY KEY,
    byte_len INTEGER NOT NULL,
    body JSONB NOT NULL,
    ref_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_artifact_payloads_last_seen ON artifact_payloads(last_seen_at DESC);
