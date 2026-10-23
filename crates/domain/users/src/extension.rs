//! `Extension` registration for the users domain: schema and required flag.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

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

    fn is_required(&self) -> bool {
        true
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new("users", include_str!("../schema/users.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "name".into(),
                    "email".into(),
                    "created_at".into(),
                ]),
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
            .with_required_columns(vec![
                "key_id".into(),
                "dimension".into(),
                "value".into(),
            ]),
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

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["events"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(UsersExtension);

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
