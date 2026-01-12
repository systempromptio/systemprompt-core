-- OAuth TUI Client Configuration Seed Data
-- Redirect URIs, grant types, response types, and scopes for sp_tui

INSERT INTO oauth_client_redirect_uris (client_id, redirect_uri, is_primary, created_at, updated_at)
VALUES
    ('sp_tui', 'http://127.0.0.1:9876/callback', true, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('sp_tui', 'http://localhost:9876/callback', false, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
ON CONFLICT (client_id, redirect_uri) DO NOTHING;

INSERT INTO oauth_client_grant_types (client_id, grant_type)
VALUES ('sp_tui', 'authorization_code')
ON CONFLICT (client_id, grant_type) DO NOTHING;

INSERT INTO oauth_client_response_types (client_id, response_type)
VALUES ('sp_tui', 'code')
ON CONFLICT (client_id, response_type) DO NOTHING;

INSERT INTO oauth_client_scopes (client_id, scope, created_at, updated_at)
VALUES
    ('sp_tui', 'user', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('sp_tui', 'admin', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
ON CONFLICT (client_id, scope) DO NOTHING;
