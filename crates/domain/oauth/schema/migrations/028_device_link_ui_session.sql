-- A device link can carry a person's UI session. `bridge_exchange_codes`
-- holds the PKCE-bound grant the browser approved (client, redirect, scope,
-- S256 challenge) and when the person signed in to that browser; redeeming
-- the code for a device key mints an authorization code for that grant.
-- `device_key_id` on `oauth_auth_codes` and `oauth_refresh_tokens` names the
-- device key the session was issued with; a refresh is refused once that key
-- no longer stands. Every column is nullable and none has a default, so each
-- is a catalog-only change. Same DDL as schema/bridge_exchange_codes.sql,
-- schema/oauth_auth_codes.sql and schema/oauth_refresh_tokens.sql.
ALTER TABLE bridge_exchange_codes
    ADD COLUMN IF NOT EXISTS ui_client_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS ui_redirect_uri TEXT,
    ADD COLUMN IF NOT EXISTS ui_scope TEXT,
    ADD COLUMN IF NOT EXISTS ui_code_challenge TEXT,
    ADD COLUMN IF NOT EXISTS ui_authenticated_at TIMESTAMPTZ;
ALTER TABLE oauth_auth_codes
    ADD COLUMN IF NOT EXISTS device_key_id TEXT;
ALTER TABLE oauth_refresh_tokens
    ADD COLUMN IF NOT EXISTS device_key_id TEXT;
