-- Proxy-side MCP session identity, keyed by mcp-session-id.
--
-- The identity established on an authenticated `initialize` was held in a
-- per-process HashMap; a session-only follow-up landing on another replica
-- found nothing and was silently downgraded to anonymous. The trust anchor
-- now lives in the database so every replica resolves the same identity.

CREATE TABLE IF NOT EXISTS mcp_proxy_identities (
    session_id TEXT PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    user_type TEXT NOT NULL,
    permissions JSONB NOT NULL,
    auth_token TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP + INTERVAL '24 hours')
);

CREATE INDEX IF NOT EXISTS idx_mcp_proxy_identities_expires_at ON mcp_proxy_identities(expires_at);
