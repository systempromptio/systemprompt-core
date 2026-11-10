-- RFC 8707 resource binding for refresh tokens, and an auth-code claim that
-- records the refresh token it mints in the same statement.
-- `oauth_refresh_tokens.resource` is the resource the grant was issued for;
-- NULL means the token carries the default audiences only.
-- The claim writes `oauth_auth_codes.refresh_token_id` before that token row
-- exists, so the column is a plain reference rather than a foreign key.
BEGIN;

ALTER TABLE oauth_refresh_tokens
    ADD COLUMN IF NOT EXISTS resource TEXT;

ALTER TABLE oauth_auth_codes
    DROP CONSTRAINT IF EXISTS oauth_auth_codes_refresh_token_id_fkey;

COMMIT;
