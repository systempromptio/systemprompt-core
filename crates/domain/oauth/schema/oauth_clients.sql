CREATE TABLE IF NOT EXISTS oauth_clients (
    client_id TEXT PRIMARY KEY,
    client_secret_hash TEXT,
    client_name VARCHAR(255) NOT NULL,
    name VARCHAR(255) DEFAULT NULL,
    token_endpoint_auth_method TEXT DEFAULT 'client_secret_post',
    application_type TEXT NOT NULL DEFAULT 'web',
    client_uri TEXT,
    logo_uri TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TIMESTAMPTZ,
    owner_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_oauth_clients_active ON oauth_clients(is_active);
CREATE INDEX IF NOT EXISTS idx_oauth_clients_owner_user_id ON oauth_clients(owner_user_id);
