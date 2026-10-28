-- An API key's kind: `device` for the PAT a bridge device enrolment mints
-- (the interactive host a person installed), `user` for every other key.
-- Only a device key's bridge access is stamped as an attended session.

ALTER TABLE user_api_keys
    ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'user'
        CHECK (kind IN ('user', 'device'));
