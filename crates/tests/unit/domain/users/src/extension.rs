use systemprompt_extension::prelude::Extension;
use systemprompt_extension::{SchemaDefinition, StandingTag};
use systemprompt_test_fixtures::admitted_member;
use systemprompt_users::UsersExtension;

fn contributed_schemas() -> Vec<SchemaDefinition> {
    admitted_member(&UsersExtension, StandingTag::Core)
        .expect("the users member admits")
        .sink
        .schemas()
        .to_vec()
}

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
}

mod extension_schema_tests {
    use super::*;

    #[test]
    fn schemas_returns_expected_definitions() {
        let schemas = contributed_schemas();

        let names: Vec<&str> = schemas.iter().filter_map(|s| s.table.as_deref()).collect();
        assert_eq!(
            names,
            vec![
                "users",
                "user_sessions",
                "session_traffic_rollup_hourly",
                "banned_ips",
                "user_api_keys",
                "user_api_key_scopes",
                "user_device_certs",
                "user_device_push_tokens",
                "user_rate_limit_buckets",
                "api_key_sessions",
                "federated_identities",
                "sso_login_events",
            ]
        );
    }

    #[test]
    fn schemas_include_users_table() {
        let schemas = contributed_schemas();

        let users_schema = schemas.iter().find(|s| s.table.as_deref() == Some("users"));
        users_schema.expect("expected Some value");
    }

    #[test]
    fn schemas_include_user_sessions_table() {
        let schemas = contributed_schemas();

        let sessions_schema = schemas
            .iter()
            .find(|s| s.table.as_deref() == Some("user_sessions"));
        sessions_schema.expect("expected Some value");
    }

    #[test]
    fn schemas_include_banned_ips_table() {
        let schemas = contributed_schemas();

        let banned_schema = schemas
            .iter()
            .find(|s| s.table.as_deref() == Some("banned_ips"));
        banned_schema.expect("expected Some value");
    }

    #[test]
    fn users_schema_has_required_columns() {
        let schemas = contributed_schemas();

        let users_schema = schemas
            .iter()
            .find(|s| s.table.as_deref() == Some("users"))
            .unwrap();
        let required = &users_schema.required_columns;

        assert!(required.contains(&"id".to_string()));
        assert!(required.contains(&"name".to_string()));
        assert!(required.contains(&"email".to_string()));
        assert!(required.contains(&"created_at".to_string()));
    }

    #[test]
    fn user_sessions_schema_has_required_columns() {
        let schemas = contributed_schemas();

        let sessions_schema = schemas
            .iter()
            .find(|s| s.table.as_deref() == Some("user_sessions"))
            .unwrap();
        let required = &sessions_schema.required_columns;

        assert!(required.contains(&"session_id".to_string()));
        assert!(required.contains(&"started_at".to_string()));
    }

    #[test]
    fn banned_ips_schema_has_required_columns() {
        let schemas = contributed_schemas();

        let banned_schema = schemas
            .iter()
            .find(|s| s.table.as_deref() == Some("banned_ips"))
            .unwrap();
        let required = &banned_schema.required_columns;

        assert!(required.contains(&"ip_address".to_string()));
        assert!(required.contains(&"reason".to_string()));
        assert!(required.contains(&"banned_at".to_string()));
    }

    #[test]
    fn all_schemas_have_inline_sql() {
        let schemas = contributed_schemas();

        for schema in &schemas {
            assert!(
                !schema.sql.is_empty(),
                "Schema {} has empty SQL",
                schema.table.as_deref().unwrap_or("<table-less>")
            );
        }
    }
}

mod extension_trait_tests {
    use super::*;

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

    #[test]
    fn extension_contributes_schemas() {
        assert!(!contributed_schemas().is_empty());
    }
}

#[test]
fn session_expiry_is_declared_without_reporting_privacy() {
    let schemas = contributed_schemas();
    assert!(
        schemas
            .iter()
            .all(|schema| !schema.sql.contains("reporting"))
    );
    let users = schemas
        .iter()
        .find(|schema| schema.table.as_deref() == Some("users"))
        .unwrap();
    assert!(users.sql.contains("expire_user_sessions"));
}
