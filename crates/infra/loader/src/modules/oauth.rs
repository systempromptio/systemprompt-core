use std::path::PathBuf;
use systemprompt_models::modules::{
    ApiConfig, Module, ModuleSchema, ModuleSeed, SchemaSource, SeedSource,
};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "oauth".into(),
        version: "0.0.1".into(),
        display_name: "OAuth Authentication".into(),
        description: Some(
            "OAuth 2.0 authentication, authorization flows, and token management".into(),
        ),
        weight: Some(-50),
        dependencies: vec!["users".into()],
        schemas: Some(vec![
            ModuleSchema {
                table: "oauth_clients".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/oauth_clients.sql").into(),
                ),
                required_columns: vec![
                    "client_id".into(),
                    "client_secret_hash".into(),
                    "client_name".into(),
                    "redirect_uris".into(),
                    "created_at".into(),
                    "updated_at".into(),
                ],
            },
            ModuleSchema {
                table: "oauth_client_redirect_uris".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/oauth_client_redirect_uris.sql")
                        .into(),
                ),
                required_columns: vec!["client_id".into(), "redirect_uri".into()],
            },
            ModuleSchema {
                table: "oauth_client_grant_types".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/oauth_client_grant_types.sql")
                        .into(),
                ),
                required_columns: vec!["client_id".into(), "grant_type".into()],
            },
            ModuleSchema {
                table: "oauth_client_response_types".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/oauth_client_response_types.sql")
                        .into(),
                ),
                required_columns: vec!["client_id".into(), "response_type".into()],
            },
            ModuleSchema {
                table: "oauth_client_scopes".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/oauth_client_scopes.sql").into(),
                ),
                required_columns: vec!["client_id".into(), "scope".into()],
            },
            ModuleSchema {
                table: "oauth_client_contacts".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/oauth_client_contacts.sql")
                        .into(),
                ),
                required_columns: vec!["client_id".into(), "contact_email".into()],
            },
            ModuleSchema {
                table: "oauth_auth_codes".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/oauth_auth_codes.sql").into(),
                ),
                required_columns: vec![
                    "code".into(),
                    "client_id".into(),
                    "user_id".into(),
                    "redirect_uri".into(),
                    "scope".into(),
                    "created_at".into(),
                    "expires_at".into(),
                ],
            },
            ModuleSchema {
                table: "oauth_refresh_tokens".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/oauth_refresh_tokens.sql").into(),
                ),
                required_columns: vec![
                    "token".into(),
                    "client_id".into(),
                    "user_name".into(),
                    "scope".into(),
                    "expires_at".into(),
                    "created_at".into(),
                ],
            },
            ModuleSchema {
                table: "webauthn_credentials".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/webauthn_credentials.sql").into(),
                ),
                required_columns: vec![
                    "id".into(),
                    "user_id".into(),
                    "credential_id".into(),
                    "public_key".into(),
                    "counter".into(),
                    "display_name".into(),
                    "device_type".into(),
                    "created_at".into(),
                ],
            },
            ModuleSchema {
                table: "webauthn_challenges".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/oauth/schema/webauthn_challenges.sql").into(),
                ),
                required_columns: vec![
                    "challenge".into(),
                    "user_id".into(),
                    "challenge_type".into(),
                    "session_state".into(),
                    "expires_at".into(),
                    "created_at".into(),
                ],
            },
        ]),
        seeds: Some(vec![
            ModuleSeed {
                sql: SeedSource::Inline(
                    include_str!("../../../../domain/oauth/src/queries/seed/webauthn_client.sql")
                        .into(),
                ),
                table: "oauth_clients".into(),
                check_column: "client_id".into(),
                check_value: "sp_web".into(),
            },
            ModuleSeed {
                sql: SeedSource::Inline(
                    include_str!(
                        "../../../../domain/oauth/src/queries/seed/webauthn_client_scopes.sql"
                    )
                    .into(),
                ),
                table: "oauth_client_scopes".into(),
                check_column: "client_id".into(),
                check_value: "sp_web".into(),
            },
        ]),
        permissions: None,
        audience: vec![],
        enabled: true,
        api: Some(ApiConfig {
            enabled: true,
            path_prefix: Some("/api/v1/oauth".into()),
            openapi_path: Some("/api/v1/oauth/docs".into()),
        }),
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "oauth-module-0001-0001-000000000001".into()
}
