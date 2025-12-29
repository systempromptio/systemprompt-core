CREATE TABLE IF NOT EXISTS oauth_client_redirect_uris (
    client_id VARCHAR(255) NOT NULL,
    redirect_uri TEXT NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (client_id, redirect_uri),
    FOREIGN KEY (client_id) REFERENCES oauth_clients(client_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_oauth_client_redirect_uris_client_id
    ON oauth_client_redirect_uris(client_id);
