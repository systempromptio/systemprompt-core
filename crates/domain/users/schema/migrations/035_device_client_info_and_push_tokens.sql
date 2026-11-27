-- @cost: rows=200000 measured=51ms triggers=live
-- A device record says what it runs: the platform, the operating-system and
-- app versions, the app identifier and when it was last seen. Records written
-- before a device reported itself read platform 'unknown'; the constant
-- default makes the column a catalog change, not a rewrite. The platform
-- check is added NOT VALID under a brief lock and validated under SHARE
-- UPDATE EXCLUSIVE, so reads and writes continue. user_device_push_tokens is
-- a new, empty table whose foreign keys point at user_device_certs and users,
-- neither of them a large table.
ALTER TABLE user_device_certs
    ADD COLUMN IF NOT EXISTS platform TEXT NOT NULL DEFAULT 'unknown',
    ADD COLUMN IF NOT EXISTS os_version TEXT,
    ADD COLUMN IF NOT EXISTS app_version TEXT,
    ADD COLUMN IF NOT EXISTS app_id TEXT,
    ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ;
ALTER TABLE user_device_certs
    ADD CONSTRAINT user_device_certs_platform_check
    CHECK (platform IN ('ios', 'android', 'macos', 'windows', 'linux', 'unknown')) NOT VALID;
ALTER TABLE user_device_certs VALIDATE CONSTRAINT user_device_certs_platform_check;
CREATE TABLE IF NOT EXISTS user_device_push_tokens (
    device_id TEXT NOT NULL REFERENCES user_device_certs(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('apns', 'fcm')),
    token TEXT NOT NULL,
    app_id TEXT NOT NULL,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (device_id, provider)
);
