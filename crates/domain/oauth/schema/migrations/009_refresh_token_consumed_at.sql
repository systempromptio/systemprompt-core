-- Refresh-token reuse detection (RFC 6749 §10.4, RFC 6819 §5.2.2.3).
-- A consumed refresh token is retained as a tombstone instead of deleted so a
-- replay can be distinguished from "token never existed" and trigger
-- family-wide revocation. Tombstones are purged by
-- `cleanup_expired_refresh_tokens` once they are also past `expires_at`.
BEGIN;

ALTER TABLE oauth_refresh_tokens
    ADD COLUMN IF NOT EXISTS consumed_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_oauth_refresh_tokens_consumed_at
    ON oauth_refresh_tokens(consumed_at);

COMMIT;
