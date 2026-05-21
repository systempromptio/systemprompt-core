CREATE TABLE IF NOT EXISTS oauth_jti_revocations (
    jti         TEXT        PRIMARY KEY,
    user_id     UUID        NOT NULL,
    revoked_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    exp         TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS oauth_jti_revocations_exp_idx ON oauth_jti_revocations (exp);
CREATE INDEX IF NOT EXISTS oauth_jti_revocations_user_idx ON oauth_jti_revocations (user_id);
