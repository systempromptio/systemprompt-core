ALTER TABLE user_contexts ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'user';

UPDATE user_contexts SET kind = 'cli_session' WHERE name LIKE 'CLI Session - %';
