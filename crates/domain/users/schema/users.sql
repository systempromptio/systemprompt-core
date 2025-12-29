CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    full_name VARCHAR(255),
    display_name VARCHAR(255),
    status TEXT NOT NULL CHECK(status IN ('active', 'inactive', 'suspended', 'pending', 'deleted', 'temporary')) DEFAULT 'active',
    email_verified BOOLEAN NOT NULL DEFAULT false,
    roles TEXT[] NOT NULL DEFAULT ARRAY['user']::TEXT[],
    is_bot BOOLEAN NOT NULL DEFAULT false,
    is_scanner BOOLEAN NOT NULL DEFAULT false,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_name ON users(name);
CREATE INDEX IF NOT EXISTS idx_users_bot_status ON users(is_bot, is_scanner);
