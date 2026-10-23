-- The gateway session an API key's requests land on when the caller sends no
-- x-session-id the gateway issued. One row per key: the first such request
-- mints a user_sessions row and binds it here, and every later one reuses it.
-- session_id carries no foreign key, so binding never locks user_sessions; a
-- binding whose session has gone is re-minted on the next request.
CREATE TABLE IF NOT EXISTS api_key_sessions (
    api_key_id TEXT PRIMARY KEY REFERENCES user_api_keys(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    bound_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
