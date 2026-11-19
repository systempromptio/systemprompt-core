-- Person UI clients (services/identity/*.yaml `person_ui: true`) and the
-- sign-in time a refresh token carries forward.
-- `oauth_clients.person_ui` is set only by the identity plane for a declared
-- client; the issuer honours it only while `declared_source` is set.
-- `oauth_refresh_tokens.authenticated_at` is when the person signed in for
-- the grant the token belongs to; rotation copies it, so a UI session's
-- `auth_time` and its absolute cap survive every refresh. Rows that predate it
-- take the migration time, which no non-UI client reads.
-- Constant defaults are catalog-only changes. Same DDL as
-- schema/oauth_clients.sql and schema/oauth_refresh_tokens.sql.
ALTER TABLE oauth_clients
    ADD COLUMN IF NOT EXISTS person_ui BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE oauth_refresh_tokens
    ADD COLUMN IF NOT EXISTS authenticated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;
