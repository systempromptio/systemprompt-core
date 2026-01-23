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
        20
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
            .with_required_columns(vec![
                "id".into(),
                "client_id".into(),
                "redirect_uri".into(),
            ]),
            SchemaDefinition::inline(
                "oauth_client_grant_types",
                include_str!("../schema/oauth_client_grant_types.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "client_id".into(),
                "grant_type".into(),
            ]),
            SchemaDefinition::inline(
                "oauth_client_response_types",
                include_str!("../schema/oauth_client_response_types.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "client_id".into(),
                "response_type".into(),
            ]),
            SchemaDefinition::inline(
                "oauth_client_scopes",
                include_str!("../schema/oauth_client_scopes.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "client_id".into(),
                "scope".into(),
            ]),
            SchemaDefinition::inline(
                "oauth_client_contacts",
                include_str!("../schema/oauth_client_contacts.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "client_id".into(),
                "contact".into(),
            ]),
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
                "token".into(),
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
            .with_required_columns(vec!["id".into(), "challenge".into()]),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }
}

register_extension!(OauthExtension);
