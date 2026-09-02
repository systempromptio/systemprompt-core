ALTER TABLE webauthn_challenges ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE webauthn_challenges DROP CONSTRAINT IF EXISTS webauthn_challenges_user_id_fkey;
ALTER TABLE webauthn_challenges ALTER COLUMN session_state TYPE JSONB USING session_state::jsonb;
ALTER TABLE webauthn_challenges ADD COLUMN IF NOT EXISTS oauth_state TEXT;
