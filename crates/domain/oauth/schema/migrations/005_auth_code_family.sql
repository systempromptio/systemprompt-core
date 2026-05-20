-- Refresh-token family + auth-code linkage for replay detection.
-- An auth code, the refresh token it minted, and every rotation derived from
-- that refresh token share a `family_id`. Reuse of a consumed auth code
-- revokes the entire family (RFC 6819 §5.2.2.3 "refresh token rotation").
BEGIN;

ALTER TABLE oauth_refresh_tokens
    ADD COLUMN family_id TEXT;

UPDATE oauth_refresh_tokens SET family_id = token_id WHERE family_id IS NULL;

ALTER TABLE oauth_refresh_tokens
    ALTER COLUMN family_id SET NOT NULL;

CREATE INDEX idx_oauth_refresh_tokens_family_id ON oauth_refresh_tokens(family_id);

ALTER TABLE oauth_auth_codes
    ADD COLUMN refresh_token_id TEXT
    REFERENCES oauth_refresh_tokens(token_id) ON DELETE SET NULL;

CREATE INDEX idx_oauth_auth_codes_refresh_token_id ON oauth_auth_codes(refresh_token_id);

COMMIT;
