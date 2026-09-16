-- The proxy-side session identity carries the caller's roles so a
-- session-only follow-up presents the same authz subject as the
-- authenticated initialize that established it.

ALTER TABLE mcp_proxy_identities
    ADD COLUMN IF NOT EXISTS roles JSONB NOT NULL DEFAULT '[]'::jsonb;
