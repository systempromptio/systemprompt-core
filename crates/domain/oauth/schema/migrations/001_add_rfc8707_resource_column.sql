
ALTER TABLE oauth_auth_codes ADD COLUMN IF NOT EXISTS resource TEXT;
