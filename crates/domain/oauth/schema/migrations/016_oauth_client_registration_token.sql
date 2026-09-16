ALTER TABLE oauth_clients ADD COLUMN IF NOT EXISTS registration_token_hash TEXT;
