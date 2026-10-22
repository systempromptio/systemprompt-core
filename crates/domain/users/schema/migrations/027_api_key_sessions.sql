-- Why: binds an API key to the gateway session minted for it (credential.md
-- §4, session id bound to the credential). Creates one new, empty table whose
-- only foreign key points at user_api_keys, a small table: the FK takes a
-- SHARE ROW EXCLUSIVE lock on user_api_keys for the duration of the CREATE
-- (milliseconds, no scan). It touches neither user_sessions nor ai_requests,
-- so a 100M-row ai_requests is not locked at all.
CREATE TABLE IF NOT EXISTS api_key_sessions (
    api_key_id TEXT PRIMARY KEY REFERENCES user_api_keys(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    bound_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
