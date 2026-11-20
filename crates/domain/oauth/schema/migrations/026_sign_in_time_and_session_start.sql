-- When the person signed in and when the session began are two times.
-- `oauth_auth_codes.authenticated_at` is when the person presented a
-- credential for the code; `created_at` stays when the code was issued.
-- `oauth_refresh_tokens.session_started_at` is when the code the family
-- descends from was issued, the anchor of a UI session's absolute cap;
-- `authenticated_at` stays the sign-in the access token reports as
-- `auth_time`. Rotation copies both.
-- Until this migration one time played both roles, the code's `created_at`,
-- so each new column is backfilled from the value its row already carries.
-- Neither table is in scripts/large-tables.txt: codes live ten minutes and
-- refresh tokens are swept at expiry. Same DDL as schema/oauth_auth_codes.sql
-- and schema/oauth_refresh_tokens.sql.
ALTER TABLE oauth_auth_codes
    ADD COLUMN IF NOT EXISTS authenticated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;
UPDATE oauth_auth_codes SET authenticated_at = created_at;
ALTER TABLE oauth_refresh_tokens
    ADD COLUMN IF NOT EXISTS session_started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;
UPDATE oauth_refresh_tokens SET session_started_at = authenticated_at;
