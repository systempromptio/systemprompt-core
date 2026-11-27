-- The push token a provider issued to the app on one device record. A row
-- belongs to its device and dies with it; user_id repeats the device's owner
-- so a user's tokens are read and purged by one key. A token addresses one
-- app install, so a provider's token is held by one device at a time: the
-- unique index is over the token's digest because a token may be longer than
-- a btree entry.
CREATE TABLE IF NOT EXISTS user_device_push_tokens (
    device_id TEXT NOT NULL REFERENCES user_device_certs(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('apns', 'fcm')),
    token TEXT NOT NULL,
    app_id TEXT NOT NULL,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (device_id, provider)
);
CREATE INDEX IF NOT EXISTS idx_user_device_push_tokens_user
    ON user_device_push_tokens(user_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_device_push_tokens_token
    ON user_device_push_tokens(provider, md5(token));
