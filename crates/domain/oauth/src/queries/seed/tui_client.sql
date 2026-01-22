-- OAuth TUI Client Seed Data
-- TUI client for the systemprompt.io Terminal User Interface

INSERT INTO oauth_clients (
    client_id,
    client_secret_hash,
    client_name,
    token_endpoint_auth_method,
    is_active,
    created_at,
    updated_at
) VALUES (
    'sp_tui',
    '$2b$12$sp_tui.client.secret.hash.value.for.authentication',
    'systemprompt.io TUI',
    'none',
    true,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
) ON CONFLICT (client_id) DO NOTHING;
