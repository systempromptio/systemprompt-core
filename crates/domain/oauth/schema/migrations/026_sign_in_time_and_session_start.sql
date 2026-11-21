-- When the person signed in and when the session began are two times.
-- `oauth_auth_codes.authenticated_at` is when the person presented a
-- credential for the code; `created_at` stays when the code was issued.
-- `oauth_refresh_tokens.session_started_at` is when the code the family
-- descends from was issued, the anchor of a UI session's absolute cap;
-- `authenticated_at` stays the sign-in the access token reports as
-- `auth_time`. Rotation copies both.
-- Rows that predate the columns take the migration time: a code lives ten
-- minutes, and a UI session open across the upgrade counts its cap from the
-- upgrade once while its `auth_time` is untouched.
-- Constant defaults are catalog-only changes. Same DDL as
-- schema/oauth_auth_codes.sql and schema/oauth_refresh_tokens.sql.
ALTER TABLE oauth_auth_codes
    ADD COLUMN IF NOT EXISTS authenticated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE oauth_refresh_tokens
    ADD COLUMN IF NOT EXISTS session_started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;
