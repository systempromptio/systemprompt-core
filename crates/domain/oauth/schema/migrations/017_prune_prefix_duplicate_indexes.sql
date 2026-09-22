-- Drop every index on this crate's tables whose column list is a strict
-- prefix of another index on the same table. The `oauth_client_*` child
-- tables each keep a `client_id` index that is the leading column of their
-- own primary key, and two lookup indexes duplicate a UNIQUE constraint
-- outright.
--
-- Each one names the index that already covers it. None is UNIQUE and none
-- backs a constraint.

-- covered by bridge_user_host_model_prefs_pkey (user_id, host_id)
DROP INDEX IF EXISTS idx_bridge_user_host_model_prefs_user;
-- covered by bridge_user_host_prefs_pkey (user_id, host_id)
DROP INDEX IF EXISTS idx_bridge_user_host_prefs_user;

-- each covered by its table's primary key, whose leading column is client_id
DROP INDEX IF EXISTS idx_oauth_client_contacts_client_id;
DROP INDEX IF EXISTS idx_oauth_client_grant_types_client_id;
DROP INDEX IF EXISTS idx_oauth_client_redirect_uris_client_id;
DROP INDEX IF EXISTS idx_oauth_client_response_types_client_id;
DROP INDEX IF EXISTS idx_oauth_client_scopes_client_id;

-- covered by webauthn_credentials_credential_id_key (same column)
DROP INDEX IF EXISTS idx_webauthn_credentials_credential_id;
-- covered by webauthn_setup_tokens_token_hash_key (same column)
DROP INDEX IF EXISTS idx_webauthn_setup_tokens_token_hash;
