//! OAuth extension registration metadata.

use systemprompt_extension::prelude::*;

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
            SchemaDefinition::new("oauth_clients", include_str!("../schema/oauth_clients.sql"))
                .with_required_columns(vec![
                    "client_id".into(),
                    "client_name".into(),
                    "created_at".into(),
                ]),
            SchemaDefinition::new(
                "oauth_client_redirect_uris",
                include_str!("../schema/oauth_client_redirect_uris.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "redirect_uri".into()]),
            SchemaDefinition::new(
                "oauth_client_grant_types",
                include_str!("../schema/oauth_client_grant_types.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "grant_type".into()]),
            SchemaDefinition::new(
                "oauth_client_response_types",
                include_str!("../schema/oauth_client_response_types.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "response_type".into()]),
            SchemaDefinition::new(
                "oauth_client_scopes",
                include_str!("../schema/oauth_client_scopes.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "scope".into()]),
            SchemaDefinition::new(
                "oauth_client_contacts",
                include_str!("../schema/oauth_client_contacts.sql"),
            )
            .with_required_columns(vec!["client_id".into(), "contact_email".into()]),
            SchemaDefinition::new(
                "oauth_auth_codes",
                include_str!("../schema/oauth_auth_codes.sql"),
            )
            .with_required_columns(vec![
                "code".into(),
                "client_id".into(),
                "user_id".into(),
            ]),
            SchemaDefinition::new(
                "oauth_refresh_tokens",
                include_str!("../schema/oauth_refresh_tokens.sql"),
            )
            .with_required_columns(vec![
                "token_id".into(),
                "client_id".into(),
                "user_id".into(),
            ]),
            SchemaDefinition::new(
                "webauthn_credentials",
                include_str!("../schema/webauthn_credentials.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "user_id".into(),
                "credential_id".into(),
            ]),
            SchemaDefinition::new(
                "webauthn_challenges",
                include_str!("../schema/webauthn_challenges.sql"),
            )
            .with_required_columns(vec!["challenge".into(), "user_id".into()]),
            SchemaDefinition::new(
                "webauthn_setup_tokens",
                include_str!("../schema/webauthn_setup_tokens.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "user_id".into(),
                "token_hash".into(),
            ]),
            SchemaDefinition::new(
                "bridge_exchange_codes",
                include_str!("../schema/bridge_exchange_codes.sql"),
            )
            .with_required_columns(vec![
                "code_hash".into(),
                "user_id".into(),
                "expires_at".into(),
            ]),
            SchemaDefinition::new(
                "bridge_sessions",
                include_str!("../schema/bridge_sessions.sql"),
            )
            .with_required_columns(vec![
                "session_id".into(),
                "user_id".into(),
                "bridge_version".into(),
                "last_heartbeat_at".into(),
            ]),
            SchemaDefinition::new(
                "bridge_user_host_prefs",
                include_str!("../schema/bridge_user_host_prefs.sql"),
            )
            .with_required_columns(vec![
                "user_id".into(),
                "host_id".into(),
                "enabled".into(),
            ]),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn owned_tables(&self) -> Vec<&'static str> {
        vec!["oauth_auth_codes"]
    }
}

register_extension!(OauthExtension);
