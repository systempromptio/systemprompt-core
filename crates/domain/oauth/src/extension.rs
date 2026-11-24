//! The oauth member: its client, token, `WebAuthn` and bridge schemas, its
//! migrations, the expired-grant cleanup job, the sealed browser-session
//! issuer (`DynSessionIssuer`), which requires the users member's sealed user
//! directory (`DynUserProvider`), its repositories
//! (`Arc<OAuthRepositories>`) and the plugin-token minter
//! (`DynPluginTokenMinter`), both conferred on `identity-http`, the sealed
//! token revocation check (`DynTokenRevocations`, a core-only provide the
//! kernel binds into the users member's revocation cell) and the sealed
//! `DynBridgeHostFilterMigration` `admin config migrate --data` runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;

use systemprompt_database::DbPool;
use systemprompt_extension::reserved::ConferralRow;
use systemprompt_models::auth::DynTokenRevocations;
use systemprompt_traits::{CacheEpochView, DynBridgeHostFilterMigration};

use crate::jobs::OauthCleanupJob;
use crate::repository::{BridgeHostPrefsRepository, JtiRevocationChecker};

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

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in client_schemas()
            .into_iter()
            .chain(token_schemas())
            .chain(webauthn_schemas())
            .chain(bridge_schemas())
        {
            c.schema(schema);
        }
        c.migrations(extension_migrations!())
            .requires::<systemprompt_traits::DynUserProvider>()
            .provide_conferred(
                ConferralRow::IdentityOAuthRepositories,
                crate::provides::oauth_repositories,
            )
            .provide_sealed_after_governance(crate::provides::session_issuer)
            .provide_conferred_after_governance(
                ConferralRow::IdentityPluginTokens,
                crate::provides::plugin_token_minter,
            )
            .job(OauthCleanupJob)
            .provide_sealed(
                |r: &Resolved<'_>| -> Result<DynTokenRevocations, ProvideError> {
                    let db = r.get::<DbPool>()?;
                    let epochs = r.get::<CacheEpochView>()?;
                    Ok(Arc::new(JtiRevocationChecker::new(&db, epochs)))
                },
            )
            .provide_sealed(
                |r: &Resolved<'_>| -> Result<DynBridgeHostFilterMigration, ProvideError> {
                    let db = r.get::<DbPool>()?;
                    Ok(Arc::new(BridgeHostPrefsRepository::new(&db)))
                },
            );
        c.user_purge(UserPurgeTable {
            table: "oauth_jti_revocations",
            column: "user_id",
            predicate: None,
        });
        Ok(())
    }
}

fn client_schemas() -> Vec<SchemaDefinition> {
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
    ]
}

fn token_schemas() -> Vec<SchemaDefinition> {
    vec![
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
            "oauth_auth_codes",
            include_str!("../schema/oauth_auth_codes.sql"),
        )
        .with_required_columns(vec!["code".into(), "client_id".into(), "user_id".into()]),
        SchemaDefinition::new(
            "oauth_state_bindings",
            include_str!("../schema/oauth_state_bindings.sql"),
        )
        .with_required_columns(vec![
            "state_token_hash".into(),
            "return_to".into(),
            "expires_at".into(),
        ]),
        SchemaDefinition::new(
            "oauth_jti_revocations",
            include_str!("../schema/oauth_jti_revocations.sql"),
        )
        .with_required_columns(vec!["jti".into(), "user_id".into(), "exp".into()]),
        SchemaDefinition::new(
            "oauth_consents",
            include_str!("../schema/oauth_consents.sql"),
        )
        .with_required_columns(vec!["user_id".into(), "client_id".into(), "scopes".into()]),
        SchemaDefinition::sql_only(include_str!(
            "../schema/oauth_jti_revocations_cache_notify.sql"
        )),
    ]
}

fn webauthn_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "webauthn_credentials",
            include_str!("../schema/webauthn_credentials.sql"),
        )
        .with_required_columns(vec!["id".into(), "user_id".into(), "credential_id".into()]),
        SchemaDefinition::new(
            "webauthn_challenges",
            include_str!("../schema/webauthn_challenges.sql"),
        )
        .with_required_columns(vec!["challenge".into(), "user_id".into()]),
        SchemaDefinition::new(
            "webauthn_setup_tokens",
            include_str!("../schema/webauthn_setup_tokens.sql"),
        )
        .with_required_columns(vec!["id".into(), "user_id".into(), "token_hash".into()]),
    ]
}

fn bridge_schemas() -> Vec<SchemaDefinition> {
    vec![
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
        .with_required_columns(vec!["user_id".into(), "host_id".into(), "enabled".into()]),
        SchemaDefinition::new(
            "bridge_user_host_model_prefs",
            include_str!("../schema/bridge_user_host_model_prefs.sql"),
        )
        .with_required_columns(vec![
            "user_id".into(),
            "host_id".into(),
            "model_protocols".into(),
        ]),
        SchemaDefinition::new("id_jag_replay", include_str!("../schema/id_jag_replay.sql"))
            .with_required_columns(vec!["jti".into(), "expires_at".into()]),
        SchemaDefinition::new(
            "sso_state_bindings",
            include_str!("../schema/sso_state_bindings.sql"),
        )
        .with_required_columns(vec![
            "state_hash".into(),
            "binder_hash".into(),
            "expires_at".into(),
        ]),
        SchemaDefinition::new(
            "sso_replay_guard",
            include_str!("../schema/sso_replay_guard.sql"),
        )
        .with_required_columns(vec!["scope".into(), "id".into(), "expires_at".into()]),
    ]
}
