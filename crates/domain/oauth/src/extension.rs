use systemprompt_extension::prelude::*;

const MIGRATION_001_RFC8707_RESOURCE: &str = r"
ALTER TABLE oauth_auth_codes ADD COLUMN IF NOT EXISTS resource TEXT;
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
        30
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
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration::new(
            1,
            "add_rfc8707_resource_column",
            MIGRATION_001_RFC8707_RESOURCE,
        )]
    }
}

register_extension!(OauthExtension);
