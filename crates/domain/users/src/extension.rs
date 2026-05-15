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

    fn migration_weight(&self) -> u32 {
        100
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
                .with_required_columns(vec!["session_id".into(), "started_at".into()]),
            SchemaDefinition::new("banned_ips", include_str!("../schema/banned_ips.sql"))
                .with_required_columns(vec![
                    "ip_address".into(),
                    "reason".into(),
                    "banned_at".into(),
                ]),
            SchemaDefinition::new(
                "session_analytics_views",
                include_str!("../schema/session_analytics_views.sql"),
            ),
            SchemaDefinition::new(
                "referrer_analytics_views",
                include_str!("../schema/referrer_analytics_views.sql"),
            ),
            SchemaDefinition::new(
                "bot_analytics_views",
                include_str!("../schema/bot_analytics_views.sql"),
            ),
            SchemaDefinition::new("user_api_keys", include_str!("../schema/user_api_keys.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "user_id".into(),
                    "key_prefix".into(),
                    "key_hash".into(),
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
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn owned_tables(&self) -> Vec<&'static str> {
        vec!["user_sessions"]
    }
}

register_extension!(UsersExtension);
