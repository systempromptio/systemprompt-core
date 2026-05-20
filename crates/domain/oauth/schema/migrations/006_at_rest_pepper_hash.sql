-- Invalidate any OAuth auth codes and refresh tokens left from before the
-- at-rest pepper landed. From this migration onward, oauth_auth_codes.code,
-- oauth_auth_codes.refresh_token_id, oauth_refresh_tokens.token_id, and
-- oauth_refresh_tokens.family_id (when seeded from token_id) hold the
-- lowercase-hex HMAC-SHA-256 of the raw identifier under the deployment
-- pepper. Existing rows are plaintext and would never match a hashed lookup,
-- so they are discarded; active clients re-authenticate on next request.
BEGIN;

DELETE FROM oauth_refresh_tokens;
DELETE FROM oauth_auth_codes;

COMMIT;
