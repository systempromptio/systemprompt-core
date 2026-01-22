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
        10
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline("users", include_str!("../schema/users.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "name".into(),
                    "email".into(),
                    "created_at".into(),
                ]),
            SchemaDefinition::inline("user_sessions", include_str!("../schema/user_sessions.sql"))
                .with_required_columns(vec![
                    "session_id".into(),
                    "started_at".into(),
                ]),
            SchemaDefinition::inline("banned_ips", include_str!("../schema/banned_ips.sql"))
                .with_required_columns(vec![
                    "ip_address".into(),
                    "reason".into(),
                    "banned_at".into(),
                ]),
            SchemaDefinition::inline(
                "session_analytics_views",
                include_str!("../schema/session_analytics_views.sql"),
            ),
            SchemaDefinition::inline(
                "referrer_analytics_views",
                include_str!("../schema/referrer_analytics_views.sql"),
            ),
            SchemaDefinition::inline(
                "bot_analytics_views",
                include_str!("../schema/bot_analytics_views.sql"),
            ),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }
}

register_extension!(UsersExtension);
