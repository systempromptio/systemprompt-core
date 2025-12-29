-- OAuth WebAuthn Client Scopes Seed Data
-- Scopes for the sp_web client to allow user authentication

INSERT INTO oauth_client_scopes (client_id, scope, created_at, updated_at) VALUES
    ('sp_web', 'user', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('sp_web', 'admin', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
ON CONFLICT (client_id, scope) DO NOTHING;
