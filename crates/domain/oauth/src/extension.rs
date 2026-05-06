//! OAuth extension registration metadata.

use systemprompt_extension::prelude::*;

const MIGRATION_001_RFC8707_RESOURCE: &str = r"
ALTER TABLE oauth_auth_codes ADD COLUMN IF NOT EXISTS resource TEXT;
";

const MIGRATION_002_RENAME_COWORK_TO_BRIDGE: &str = r"
DO $$
BEGIN
    IF to_regclass('cowork_exchange_codes') IS NOT NULL
       AND to_regclass('bridge_exchange_codes') IS NULL THEN
        ALTER TABLE cowork_exchange_codes RENAME TO bridge_exchange_codes;
    END IF;
    IF to_regclass('idx_cowork_exchange_codes_user') IS NOT NULL THEN
        ALTER INDEX idx_cowork_exchange_codes_user RENAME TO idx_bridge_exchange_codes_user;
    END IF;
    IF to_regclass('idx_cowork_exchange_codes_active') IS NOT NULL THEN
        ALTER INDEX idx_cowork_exchange_codes_active RENAME TO idx_bridge_exchange_codes_active;
    END IF;
END $$;
";

#[derive(Debug, Clone, Copy, Default)]
pub struct OauthExtension;

impl Extension for OauthExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "oauth",
            name: "OAuth",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn migration_weight(&self) -> u32 {
        300
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline("oauth_clients", include_str!("../schema/oauth_clients.sql"))
                .with_required_columns(vec![
                    "client_id".into(),
                    "client_name".into(),
                    "created_at".into(),
                ]),
            SchemaDefinition::inline(
                "oauth_client_redirect_uris",
                include_str!("../schema/oauth_client_redirect_uris.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "redirect_uri".into()]),
            SchemaDefinition::inline(
                "oauth_client_grant_types",
                include_str!("../schema/oauth_client_grant_types.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "grant_type".into()]),
            SchemaDefinition::inline(
                "oauth_client_response_types",
                include_str!("../schema/oauth_client_response_types.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "response_type".into()]),
            SchemaDefinition::inline(
                "oauth_client_scopes",
                include_str!("../schema/oauth_client_scopes.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "scope".into()]),
            SchemaDefinition::inline(
                "oauth_client_contacts",
                include_str!("../schema/oauth_client_contacts.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "contact_email".into()]),
            SchemaDefinition::inline(
                "oauth_auth_codes",
                include_str!("../schema/oauth_auth_codes.sql"),
            )
            .with_required_columns(vec![
                "code".into(),
                "client_id".into(),
                "user_id".into(),
            ]),
            SchemaDefinition::inline(
                "oauth_refresh_tokens",
                include_str!("../schema/oauth_refresh_tokens.sql"),
            )
            .with_required_columns(vec![
                "token_id".into(),
                "client_id".into(),
                "user_id".into(),
            ]),
            SchemaDefinition::inline(
                "webauthn_credentials",
                include_str!("../schema/webauthn_credentials.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "user_id".into(),
                "credential_id".into(),
            ]),
            SchemaDefinition::inline(
                "webauthn_challenges",
                include_str!("../schema/webauthn_challenges.sql"),
            )
            .with_required_columns(vec!["challenge".into(), "user_id".into()]),
            SchemaDefinition::inline(
                "webauthn_setup_tokens",
                include_str!("../schema/webauthn_setup_tokens.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "user_id".into(),
                "token_hash".into(),
            ]),
            SchemaDefinition::inline(
                "bridge_exchange_codes",
                include_str!("../schema/bridge_exchange_codes.sql"),
            )
            .with_required_columns(vec![
                "code_hash".into(),
                "user_id".into(),
                "expires_at".into(),
            ]),
            SchemaDefinition::inline(
                "bridge_sessions",
                include_str!("../schema/bridge_sessions.sql"),
            )
            .with_required_columns(vec![
                "session_id".into(),
                "user_id".into(),
                "bridge_version".into(),
                "last_heartbeat_at".into(),
            ]),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![
            Migration::new(
                1,
                "add_rfc8707_resource_column",
                MIGRATION_001_RFC8707_RESOURCE,
            ),
            Migration::new(
                2,
                "rename_cowork_to_bridge",
                MIGRATION_002_RENAME_COWORK_TO_BRIDGE,
            ),
        ]
    }
}

register_extension!(OauthExtension);
