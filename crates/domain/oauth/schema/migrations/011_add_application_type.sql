ALTER TABLE oauth_clients ADD COLUMN IF NOT EXISTS application_type TEXT NOT NULL DEFAULT 'web';
