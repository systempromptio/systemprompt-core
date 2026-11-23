//! The users member: its schemas and migrations, the IP-ban store, the
//! session store (`DynSessionStore`) **sealed** (it creates a session for any
//! user and revokes all of a user's sessions, so only kernel code and provide
//! fns read it: the API's session, sign-in and JWT paths), the open session
//! telemetry view (`DynSessionTelemetry`: the bot, fingerprint, counter and
//! geo operations the analytics member and the request-analytics middleware
//! use, with no create, revoke or end), the user-role audience resolver the
//! notifications and brain slots append as a core default (`CoreSeam<dyn
//! NotificationAudienceResolver, RoleAudience>`), its two maintenance jobs, the
//! role directory governance's `RoleDirectorySlot` holds, the `role`
//! subject-attribute provider, and the purge of its own user-keyed tables a
//! user delete clears. It also provides the AI session provider
//! (`DynAiSessionProvider`) **sealed**: it creates and meters a session for any
//! user it is handed, so only the `ai` member's provide reads it. The gateway
//! session mint (`DynGatewaySessionMint`) is sealed and conferred on
//! `gateway-http`. The request authenticator (`DynRequestAuthenticator`) is
//! open: it verifies a request against the kernel's verification keyring, the
//! issuer, the user and session rows and the kernel's token revocation check,
//! and exposes no create, revoke or mint; a kernel that seeds no revocation
//! check gets `UnavailableRevocations`, which refuses every token id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::NotificationAudienceResolver;
use systemprompt_extension::prelude::*;
use systemprompt_governance::authz::SharedRoleDirectory;
use systemprompt_governance::contract::RoleDirectorySlot;
use systemprompt_traits::authz::SubjectAttributeProvider;
use systemprompt_traits::core_seam::RoleAudience;
use systemprompt_traits::{
    CoreSeam, DynIpBanStore, DynSessionStore, DynSessionTelemetry, DynSessionUsageCounters,
};

use crate::jobs::{CleanupAnonymousUsersJob, UserRateLimitPruneJob};
use crate::repository::{
    BannedIpRepository, SessionRepository, UserRepository, UsersRoleDirectory,
    UsersRoleSubjectProvider,
};
use crate::services::UserRoleAudienceResolver;

type RoleAudienceSeam = CoreSeam<dyn NotificationAudienceResolver, RoleAudience>;

#[derive(Debug, Clone, Copy, Default)]
pub struct UsersExtension;

impl Extension for UsersExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "users",
            name: "Users",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in schemas() {
            c.schema(schema);
        }
        c.user_purge(UserPurgeTable {
            table: "user_rate_limit_buckets",
            column: "user_id",
            predicate: None,
        })
        .user_purge(UserPurgeTable {
            table: "sso_login_events",
            column: "user_id",
            predicate: None,
        })
        .migrations(extension_migrations!())
        .provide(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let bans: DynIpBanStore = Arc::new(BannedIpRepository::new(&db));
            Ok(bans)
        })
        .provide_sealed(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let sessions: DynSessionStore = Arc::new(SessionRepository::new(&db));
            Ok(sessions)
        })
        .provide(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let telemetry: DynSessionTelemetry = Arc::new(SessionRepository::new(&db));
            Ok(telemetry)
        })
        .provide_sealed(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let sessions: systemprompt_traits::DynAiSessionProvider = Arc::new(
                crate::UsersAiSessionProvider::from_repository(SessionRepository::new(&db)),
            );
            Ok(sessions)
        })
        .provide_conferred(
            ConferralRow::GatewaySessionMint,
            crate::provides::gateway_session_mint,
        )
        .provide(crate::provides::request_authenticator)
        .provide(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let usage: DynSessionUsageCounters = Arc::new(SessionRepository::new(&db));
            Ok(usage)
        })
        .provide(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let resolver: Arc<dyn NotificationAudienceResolver> = Arc::new(
                UserRoleAudienceResolver::new(Arc::new(UserRepository::new(&db))),
            );
            Ok(RoleAudienceSeam::new(resolver))
        })
        .subject_attributes(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let provider: Arc<dyn SubjectAttributeProvider> =
                Arc::new(UsersRoleSubjectProvider::new(db.write_pool()));
            Ok(provider)
        })
        .job(UserRateLimitPruneJob)
        .job(CleanupAnonymousUsersJob);
        c.slot::<RoleDirectorySlot>().directory(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let directory: SharedRoleDirectory = UsersRoleDirectory::shared(db.write_pool());
            Ok(directory)
        });
        Ok(())
    }
}

fn schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new("users", include_str!("../schema/users.sql")).with_required_columns(
            vec![
                "id".into(),
                "name".into(),
                "email".into(),
                "created_at".into(),
            ],
        ),
        SchemaDefinition::new("user_sessions", include_str!("../schema/user_sessions.sql"))
            .with_required_columns(vec![
                "session_id".into(),
                "started_at".into(),
                "bot_type".into(),
            ]),
        SchemaDefinition::sql_only(include_str!("../schema/session_bot_type.sql")),
        SchemaDefinition::new(
            "session_traffic_rollup_hourly",
            include_str!("../schema/session_traffic_rollup_hourly.sql"),
        )
        .with_required_columns(vec![
            "bucket".into(),
            "traffic_class".into(),
            "sessions".into(),
        ]),
        SchemaDefinition::new("banned_ips", include_str!("../schema/banned_ips.sql"))
            .with_required_columns(vec![
                "ip_address".into(),
                "reason".into(),
                "banned_at".into(),
            ]),
        SchemaDefinition::new("user_api_keys", include_str!("../schema/user_api_keys.sql"))
            .with_required_columns(vec![
                "id".into(),
                "user_id".into(),
                "key_prefix".into(),
                "key_hash".into(),
                "model_allowlist".into(),
                "request_window_seconds".into(),
            ]),
        SchemaDefinition::new(
            "user_api_key_scopes",
            include_str!("../schema/user_api_key_scopes.sql"),
        )
        .with_required_columns(vec!["key_id".into(), "dimension".into(), "value".into()]),
        SchemaDefinition::new(
            "user_device_certs",
            include_str!("../schema/user_device_certs.sql"),
        )
        .with_required_columns(vec![
            "id".into(),
            "user_id".into(),
            "fingerprint".into(),
            "label".into(),
        ]),
        SchemaDefinition::new(
            "user_rate_limit_buckets",
            include_str!("../schema/user_rate_limit_buckets.sql"),
        )
        .with_required_columns(vec![
            "user_id".into(),
            "scope".into(),
            "window_start".into(),
            "hits".into(),
        ]),
    ]
    .into_iter()
    .chain(std::iter::once(api_key_session_schema()))
    .chain(federation_schemas())
    .collect()
}

fn federation_schemas() -> [SchemaDefinition; 2] {
    [
        SchemaDefinition::new(
            "federated_identities",
            include_str!("../schema/federated_identities.sql"),
        )
        .with_required_columns(vec![
            "issuer".into(),
            "external_sub".into(),
            "user_id".into(),
        ]),
        SchemaDefinition::new(
            "sso_login_events",
            include_str!("../schema/sso_login_events.sql"),
        )
        .with_required_columns(vec![
            "provider_id".into(),
            "outcome".into(),
            "occurred_at".into(),
        ]),
    ]
}

fn api_key_session_schema() -> SchemaDefinition {
    SchemaDefinition::new(
        "api_key_sessions",
        include_str!("../schema/api_key_sessions.sql"),
    )
    .with_required_columns(vec!["api_key_id".into(), "session_id".into()])
}
