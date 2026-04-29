use chrono::Utc;
use systemprompt_identifiers::TenantId;
use systemprompt_sync::{
    FileBundle, FileEntry, FileManifest, SyncConfig, SyncDirection, SyncError, SyncOperationResult,
};

mod boundary_tests {
    use super::*;

    #[test]
    fn empty_tenant_id() {
        let config = SyncConfig::builder(TenantId::new(""), "https://api.com", "token", "/services").build();
        assert_eq!(config.tenant_id, "");
    }

    #[test]
    fn very_long_strings() {
        let long_string = "x".repeat(10000);
        let config = SyncConfig::builder(TenantId::new(&long_string), "https://api.com", "token", "/services").build();
        assert_eq!(config.tenant_id.len(), 10000);
    }

    #[test]
    fn special_characters_in_config() {
        let config = SyncConfig::builder(
            TenantId::new("tenant-123_special!@#"),
            "https://api.example.com/v1",
            "token+with/special=chars",
            "/path/with spaces/and-dashes",
        ).build();
        assert_eq!(config.tenant_id, "tenant-123_special!@#");
    }

    #[test]
    fn file_entry_zero_size() {
        let entry = FileEntry { path: "empty.txt".to_string(), checksum: "empty_hash".to_string(), size: 0 };
        assert_eq!(entry.size, 0);
    }

    #[test]
    fn file_entry_large_size() {
        let entry = FileEntry { path: "large.bin".to_string(), checksum: "large_hash".to_string(), size: u64::MAX };
        assert_eq!(entry.size, u64::MAX);
    }

    #[test]
    fn sync_operation_result_zero_items() {
        let result = SyncOperationResult::success("empty_sync", 0);
        assert!(result.success);
        assert_eq!(result.items_synced, 0);
    }

    #[test]
    fn sync_operation_result_large_item_count() {
        let result = SyncOperationResult::success("large_sync", usize::MAX);
        assert_eq!(result.items_synced, usize::MAX);
    }
}

mod serialization_roundtrip_tests {
    use super::*;

    #[test]
    fn file_bundle_manifest_serialization() {
        let now = Utc::now();
        let bundle = FileBundle {
            manifest: FileManifest {
                files: vec![
                    FileEntry { path: "agents/default/config.yaml".to_string(), checksum: "abc123".to_string(), size: 512 },
                    FileEntry { path: "skills/test-skill/SKILL.md".to_string(), checksum: "def456".to_string(), size: 1024 },
                ],
                timestamp: now,
                checksum: "manifest_checksum".to_string(),
            },
            data: vec![],
        };
        let json = serde_json::to_string(&bundle.manifest).expect("serialize");
        assert!(json.contains("manifest_checksum"));
    }

    #[test]
    fn file_manifest_with_many_files() {
        let files: Vec<FileEntry> = (0..1000).map(|i| FileEntry {
            path: format!("file_{}.txt", i), checksum: format!("hash_{}", i), size: i as u64,
        }).collect();
        let manifest = FileManifest { files, timestamp: Utc::now(), checksum: "large_manifest".to_string() };
        assert_eq!(manifest.files.len(), 1000);
    }
}

mod error_additional_tests {
    use super::*;

    #[test]
    fn missing_config() {
        let error = SyncError::MissingConfig("local_database_url not configured".to_string());
        assert_eq!(error.to_string(), "Missing configuration: local_database_url not configured");
    }

    #[test]
    fn missing_config_empty() {
        let error = SyncError::MissingConfig(String::new());
        assert_eq!(error.to_string(), "Missing configuration: ");
    }

    #[test]
    fn io_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let sync_error: SyncError = io_error.into();
        assert!(sync_error.to_string().contains("IO error"));
    }

    #[test]
    fn json_conversion() {
        let json_str = "{ invalid json }";
        let json_result: Result<serde_json::Value, _> = serde_json::from_str(json_str);
        let json_error = json_result.expect_err("invalid JSON should fail to parse");
        let sync_error: SyncError = json_error.into();
        assert!(sync_error.to_string().contains("JSON error"));
    }

    #[test]
    fn debug_format() {
        let error = SyncError::ApiError { status: 503, message: "Service unavailable".to_string() };
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("ApiError"));
    }
}

mod config_additional_tests {
    use super::*;

    #[test]
    fn builder_with_hostname() {
        let config = SyncConfig::builder(TenantId::new("tenant"), "https://api.com", "token", "/services")
            .with_hostname(Some("app.example.com".to_string())).build();
        assert_eq!(config.hostname, Some("app.example.com".to_string()));
    }

    #[test]
    fn builder_with_hostname_none() {
        let config = SyncConfig::builder(TenantId::new("tenant"), "https://api.com", "token", "/services")
            .with_hostname(None).build();
        assert!(config.hostname.is_none());
    }

    #[test]
    fn builder_with_sync_token() {
        let config = SyncConfig::builder(TenantId::new("tenant"), "https://api.com", "token", "/services")
            .with_sync_token(Some("sync-secret-token".to_string())).build();
        assert_eq!(config.sync_token, Some("sync-secret-token".to_string()));
    }

    #[test]
    fn builder_with_local_database_url() {
        let config = SyncConfig::builder(TenantId::new("tenant"), "https://api.com", "token", "/services")
            .with_local_database_url("postgresql://localhost:5432/testdb").build();
        assert_eq!(config.local_database_url, Some("postgresql://localhost:5432/testdb".to_string()));
    }

    #[test]
    fn builder_all_options() {
        let config = SyncConfig::builder(TenantId::new("tenant-full"), "https://api.com", "api-token", "/services")
            .with_direction(SyncDirection::Pull)
            .with_dry_run(true)
            .with_verbose(true)
            .with_hostname(Some("host.example.com".to_string()))
            .with_sync_token(Some("sync-token".to_string()))
            .with_local_database_url("postgresql://db:5432/app")
            .build();
        assert_eq!(config.tenant_id, "tenant-full");
        assert_eq!(config.direction, SyncDirection::Pull);
        assert!(config.dry_run);
        assert!(config.verbose);
    }

    #[test]
    fn config_debug() {
        let config = SyncConfig::builder(TenantId::new("tenant"), "https://api.com", "token", "/services").build();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("SyncConfig"));
    }
}
