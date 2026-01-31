//! Tests for sync configuration types

use systemprompt_sync::{SyncConfig, SyncDirection, SyncOperationResult};

mod sync_direction_tests {
    use super::*;

    #[test]
    fn push_serializes_correctly() {
        let json = serde_json::to_string(&SyncDirection::Push).unwrap();
        assert_eq!(json, "\"Push\"");
    }

    #[test]
    fn pull_serializes_correctly() {
        let json = serde_json::to_string(&SyncDirection::Pull).unwrap();
        assert_eq!(json, "\"Pull\"");
    }

    #[test]
    fn push_deserializes() {
        let dir: SyncDirection = serde_json::from_str("\"Push\"").unwrap();
        assert_eq!(dir, SyncDirection::Push);
    }

    #[test]
    fn pull_deserializes() {
        let dir: SyncDirection = serde_json::from_str("\"Pull\"").unwrap();
        assert_eq!(dir, SyncDirection::Pull);
    }

    #[test]
    fn is_clone() {
        let dir = SyncDirection::Push;
        let cloned = dir;
        assert_eq!(dir, cloned);
    }

    #[test]
    fn is_copy() {
        let dir = SyncDirection::Pull;
        let copied: SyncDirection = dir;
        assert_eq!(dir, copied);
    }

    #[test]
    fn is_eq() {
        assert_eq!(SyncDirection::Push, SyncDirection::Push);
        assert_ne!(SyncDirection::Push, SyncDirection::Pull);
    }

    #[test]
    fn is_debug() {
        let debug = format!("{:?}", SyncDirection::Push);
        assert!(debug.contains("Push"));
    }
}

mod sync_config_builder_tests {
    use super::*;

    fn test_builder() -> SyncConfig {
        SyncConfig::builder("tenant-1", "https://api.example.com", "token123", "/services")
            .build()
    }

    #[test]
    fn builder_sets_tenant_id() {
        let config = test_builder();
        assert_eq!(config.tenant_id, "tenant-1");
    }

    #[test]
    fn builder_sets_api_url() {
        let config = test_builder();
        assert_eq!(config.api_url, "https://api.example.com");
    }

    #[test]
    fn builder_sets_api_token() {
        let config = test_builder();
        assert_eq!(config.api_token, "token123");
    }

    #[test]
    fn builder_sets_services_path() {
        let config = test_builder();
        assert_eq!(config.services_path, "/services");
    }

    #[test]
    fn builder_defaults_direction_to_push() {
        let config = test_builder();
        assert_eq!(config.direction, SyncDirection::Push);
    }

    #[test]
    fn builder_defaults_dry_run_to_false() {
        let config = test_builder();
        assert!(!config.dry_run);
    }

    #[test]
    fn builder_defaults_verbose_to_false() {
        let config = test_builder();
        assert!(!config.verbose);
    }

    #[test]
    fn builder_defaults_hostname_to_none() {
        let config = test_builder();
        assert!(config.hostname.is_none());
    }

    #[test]
    fn builder_defaults_sync_token_to_none() {
        let config = test_builder();
        assert!(config.sync_token.is_none());
    }

    #[test]
    fn builder_defaults_local_database_url_to_none() {
        let config = test_builder();
        assert!(config.local_database_url.is_none());
    }

    #[test]
    fn with_direction_sets_direction() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_direction(SyncDirection::Pull)
                .build();
        assert_eq!(config.direction, SyncDirection::Pull);
    }

    #[test]
    fn with_dry_run_true() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_dry_run(true)
                .build();
        assert!(config.dry_run);
    }

    #[test]
    fn with_dry_run_false() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_dry_run(false)
                .build();
        assert!(!config.dry_run);
    }

    #[test]
    fn with_verbose_true() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_verbose(true)
                .build();
        assert!(config.verbose);
    }

    #[test]
    fn with_verbose_false() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_verbose(false)
                .build();
        assert!(!config.verbose);
    }

    #[test]
    fn with_hostname_some() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_hostname(Some("host.example.com".to_string()))
                .build();
        assert_eq!(config.hostname, Some("host.example.com".to_string()));
    }

    #[test]
    fn with_hostname_none() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_hostname(None)
                .build();
        assert!(config.hostname.is_none());
    }

    #[test]
    fn with_sync_token_some() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_sync_token(Some("sync-token-123".to_string()))
                .build();
        assert_eq!(config.sync_token, Some("sync-token-123".to_string()));
    }

    #[test]
    fn with_sync_token_none() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_sync_token(None)
                .build();
        assert!(config.sync_token.is_none());
    }

    #[test]
    fn with_local_database_url() {
        let config =
            SyncConfig::builder("t", "url", "token", "/path")
                .with_local_database_url("postgres://localhost/db")
                .build();
        assert_eq!(
            config.local_database_url,
            Some("postgres://localhost/db".to_string())
        );
    }

    #[test]
    fn builder_chain_all_options() {
        let config = SyncConfig::builder("tenant", "api", "token", "/srv")
            .with_direction(SyncDirection::Pull)
            .with_dry_run(true)
            .with_verbose(true)
            .with_hostname(Some("h".to_string()))
            .with_sync_token(Some("st".to_string()))
            .with_local_database_url("db")
            .build();

        assert_eq!(config.tenant_id, "tenant");
        assert_eq!(config.direction, SyncDirection::Pull);
        assert!(config.dry_run);
        assert!(config.verbose);
        assert_eq!(config.hostname, Some("h".to_string()));
        assert_eq!(config.sync_token, Some("st".to_string()));
        assert_eq!(config.local_database_url, Some("db".to_string()));
    }

    #[test]
    fn config_is_clone() {
        let config = test_builder();
        let cloned = config.clone();
        assert_eq!(cloned.tenant_id, config.tenant_id);
    }

    #[test]
    fn config_is_debug() {
        let config = test_builder();
        let debug = format!("{:?}", config);
        assert!(debug.contains("SyncConfig"));
    }
}

mod sync_operation_result_tests {
    use super::*;

    #[test]
    fn success_sets_operation() {
        let result = SyncOperationResult::success("test_op", 5);
        assert_eq!(result.operation, "test_op");
    }

    #[test]
    fn success_sets_success_true() {
        let result = SyncOperationResult::success("op", 5);
        assert!(result.success);
    }

    #[test]
    fn success_sets_items_synced() {
        let result = SyncOperationResult::success("op", 10);
        assert_eq!(result.items_synced, 10);
    }

    #[test]
    fn success_sets_items_skipped_zero() {
        let result = SyncOperationResult::success("op", 5);
        assert_eq!(result.items_skipped, 0);
    }

    #[test]
    fn success_sets_errors_empty() {
        let result = SyncOperationResult::success("op", 5);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn success_sets_details_none() {
        let result = SyncOperationResult::success("op", 5);
        assert!(result.details.is_none());
    }

    #[test]
    fn with_details_adds_details() {
        let result = SyncOperationResult::success("op", 5)
            .with_details(serde_json::json!({"key": "value"}));
        assert!(result.details.is_some());
        let details = result.details.unwrap();
        assert_eq!(details["key"], "value");
    }

    #[test]
    fn dry_run_sets_operation() {
        let result = SyncOperationResult::dry_run("dry_op", 3, serde_json::json!({}));
        assert_eq!(result.operation, "dry_op");
    }

    #[test]
    fn dry_run_sets_success_true() {
        let result = SyncOperationResult::dry_run("op", 3, serde_json::json!({}));
        assert!(result.success);
    }

    #[test]
    fn dry_run_sets_items_synced_zero() {
        let result = SyncOperationResult::dry_run("op", 3, serde_json::json!({}));
        assert_eq!(result.items_synced, 0);
    }

    #[test]
    fn dry_run_sets_items_skipped() {
        let result = SyncOperationResult::dry_run("op", 7, serde_json::json!({}));
        assert_eq!(result.items_skipped, 7);
    }

    #[test]
    fn dry_run_sets_errors_empty() {
        let result = SyncOperationResult::dry_run("op", 3, serde_json::json!({}));
        assert!(result.errors.is_empty());
    }

    #[test]
    fn dry_run_sets_details() {
        let details = serde_json::json!({"files": ["a.txt", "b.txt"]});
        let result = SyncOperationResult::dry_run("op", 3, details);
        assert!(result.details.is_some());
    }

    #[test]
    fn result_is_serializable() {
        let result = SyncOperationResult::success("op", 5);
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("op"));
        assert!(json.contains("5"));
    }

    #[test]
    fn result_is_deserializable() {
        let result = SyncOperationResult::success("op", 5);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SyncOperationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.operation, result.operation);
        assert_eq!(deserialized.items_synced, result.items_synced);
    }

    #[test]
    fn result_is_clone() {
        let result = SyncOperationResult::success("op", 5);
        let cloned = result.clone();
        assert_eq!(cloned.operation, result.operation);
    }

    #[test]
    fn result_is_debug() {
        let result = SyncOperationResult::success("op", 5);
        let debug = format!("{:?}", result);
        assert!(debug.contains("SyncOperationResult"));
    }
}
