-- JWT JTI revocations. Lets logout terminate a bearer before its `exp`, and
-- supports admin "kick user" by revoking every active jti for a user. The
-- `exp` column carries the token's original expiry; once `now() > exp` the
-- row is worthless because the token itself would be rejected for expiry.
CREATE TABLE IF NOT EXISTS oauth_jti_revocations (
    jti         TEXT        PRIMARY KEY,
    user_id     UUID        NOT NULL,
    revoked_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    exp         TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS oauth_jti_revocations_exp_idx ON oauth_jti_revocations (exp);
CREATE INDEX IF NOT EXISTS oauth_jti_revocations_user_idx ON oauth_jti_revocations (user_id);
