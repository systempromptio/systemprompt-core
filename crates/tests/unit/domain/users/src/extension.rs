//! Unit tests for UsersExtension.
//!
//! Tests cover:
//! - Extension trait implementation
//! - Metadata (id, name, version)
//! - Migration weight and required flag
//! - Schema definitions
//! - Dependencies

use systemprompt_extension::prelude::Extension;
use systemprompt_users::UsersExtension;

// ============================================================================
// UsersExtension Metadata Tests
// ============================================================================

mod extension_metadata_tests {
    use super::*;

    #[test]
    fn metadata_id_is_users() {
        let ext = UsersExtension;
        let metadata = ext.metadata();

        assert_eq!(metadata.id, "users");
    }

    #[test]
    fn metadata_name_is_users() {
        let ext = UsersExtension;
        let metadata = ext.metadata();

        assert_eq!(metadata.name, "Users");
    }

    #[test]
    fn metadata_version_is_not_empty() {
        let ext = UsersExtension;
        let metadata = ext.metadata();

        assert!(!metadata.version.is_empty());
    }

    #[test]
    fn metadata_version_matches_crate_version() {
        let ext = UsersExtension;
        let metadata = ext.metadata();

        assert_eq!(metadata.version, env!("CARGO_PKG_VERSION"));
    }
}

// ============================================================================
// UsersExtension Configuration Tests
// ============================================================================

mod extension_config_tests {
    use super::*;

    #[test]
    fn migration_weight_is_10() {
        let ext = UsersExtension;

        assert_eq!(ext.migration_weight(), 10);
    }

    #[test]
    fn is_required_returns_true() {
        let ext = UsersExtension;

        assert!(ext.is_required());
    }

    #[test]
    fn dependencies_is_empty() {
        let ext = UsersExtension;

        assert!(ext.dependencies().is_empty());
    }
}

// ============================================================================
// UsersExtension Schema Tests
// ============================================================================

mod extension_schema_tests {
    use super::*;

    #[test]
    fn schemas_returns_six_definitions() {
        let ext = UsersExtension;
        let schemas = ext.schemas();

        assert_eq!(schemas.len(), 6);
    }

    #[test]
    fn schemas_include_users_table() {
        let ext = UsersExtension;
        let schemas = ext.schemas();

        let users_schema = schemas.iter().find(|s| s.table_name == "users");
        assert!(users_schema.is_some());
    }

    #[test]
    fn schemas_include_user_sessions_table() {
        let ext = UsersExtension;
        let schemas = ext.schemas();

        let sessions_schema = schemas.iter().find(|s| s.table_name == "user_sessions");
        assert!(sessions_schema.is_some());
    }

    #[test]
    fn schemas_include_banned_ips_table() {
        let ext = UsersExtension;
        let schemas = ext.schemas();

        let banned_schema = schemas.iter().find(|s| s.table_name == "banned_ips");
        assert!(banned_schema.is_some());
    }

    #[test]
    fn schemas_include_analytics_views() {
        let ext = UsersExtension;
        let schemas = ext.schemas();

        let session_analytics = schemas.iter().find(|s| s.table_name == "session_analytics_views");
        let referrer_analytics = schemas.iter().find(|s| s.table_name == "referrer_analytics_views");
        let bot_analytics = schemas.iter().find(|s| s.table_name == "bot_analytics_views");

        assert!(session_analytics.is_some());
        assert!(referrer_analytics.is_some());
        assert!(bot_analytics.is_some());
    }

    #[test]
    fn users_schema_has_required_columns() {
        let ext = UsersExtension;
        let schemas = ext.schemas();

        let users_schema = schemas.iter().find(|s| s.table_name == "users").unwrap();
        let required = users_schema.required_columns.as_ref().unwrap();

        assert!(required.contains(&"id".to_string()));
        assert!(required.contains(&"name".to_string()));
        assert!(required.contains(&"email".to_string()));
        assert!(required.contains(&"created_at".to_string()));
    }

    #[test]
    fn user_sessions_schema_has_required_columns() {
        let ext = UsersExtension;
        let schemas = ext.schemas();

        let sessions_schema = schemas.iter().find(|s| s.table_name == "user_sessions").unwrap();
        let required = sessions_schema.required_columns.as_ref().unwrap();

        assert!(required.contains(&"session_id".to_string()));
        assert!(required.contains(&"started_at".to_string()));
    }

    #[test]
    fn banned_ips_schema_has_required_columns() {
        let ext = UsersExtension;
        let schemas = ext.schemas();

        let banned_schema = schemas.iter().find(|s| s.table_name == "banned_ips").unwrap();
        let required = banned_schema.required_columns.as_ref().unwrap();

        assert!(required.contains(&"ip_address".to_string()));
        assert!(required.contains(&"reason".to_string()));
        assert!(required.contains(&"banned_at".to_string()));
    }

    #[test]
    fn all_schemas_have_sql_content() {
        let ext = UsersExtension;
        let schemas = ext.schemas();

        for schema in schemas {
            assert!(schema.sql.is_some(), "Schema {} missing SQL", schema.table_name);
            let sql = schema.sql.as_ref().unwrap();
            assert!(!sql.is_empty(), "Schema {} has empty SQL", schema.table_name);
        }
    }
}

// ============================================================================
// UsersExtension Trait Implementation Tests
// ============================================================================

mod extension_trait_tests {
    use super::*;

    #[test]
    fn extension_is_debug() {
        let ext = UsersExtension;
        let debug = format!("{:?}", ext);
        assert!(debug.contains("UsersExtension"));
    }

    #[test]
    fn extension_is_clone() {
        let ext = UsersExtension;
        let cloned = ext;
        assert_eq!(ext.metadata().id, cloned.metadata().id);
    }

    #[test]
    fn extension_is_copy() {
        let ext = UsersExtension;
        let copied = ext;
        assert_eq!(ext.metadata().name, copied.metadata().name);
    }

    #[test]
    fn extension_default() {
        let ext = UsersExtension::default();
        assert_eq!(ext.metadata().id, "users");
    }

    #[test]
    fn extension_implements_extension_trait() {
        fn assert_extension<T: Extension>(_: &T) {}

        let ext = UsersExtension;
        assert_extension(&ext);
    }
}
