CREATE TABLE IF NOT EXISTS oauth_consents (
    user_id VARCHAR(255) NOT NULL,
    client_id VARCHAR(255) NOT NULL,
    scopes TEXT[] NOT NULL,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, client_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES oauth_clients(client_id) ON DELETE CASCADE,
    CHECK (cardinality(scopes) > 0)
);
CREATE INDEX IF NOT EXISTS idx_oauth_consents_client ON oauth_consents(client_id);
