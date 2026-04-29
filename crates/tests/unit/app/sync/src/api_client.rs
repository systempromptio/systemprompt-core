use systemprompt_identifiers::TenantId;
use systemprompt_sync::{SyncConfig, SyncDirection};

mod sync_api_client_tests {
    use systemprompt_sync::SyncApiClient;

    #[test]
    fn new() {
        let client =
            SyncApiClient::new("https://api.example.com", "test-token").expect("test client");
        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("SyncApiClient"));
        assert!(debug_str.contains("api.example.com"));
    }

    #[test]
    fn with_direct_sync() {
        let client = SyncApiClient::new("https://api.example.com", "test-token")
            .expect("test client")
            .with_direct_sync(
                Some("app.example.com".to_string()),
                Some("sync-token".to_string()),
            );

        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("app.example.com"));
    }

    #[test]
    fn with_direct_sync_none() {
        let client = SyncApiClient::new("https://api.example.com", "test-token")
            .expect("test client")
            .with_direct_sync(None, None);

        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("None"));
    }

}

mod registry_token_tests {
    use systemprompt_sync::api_client::RegistryToken;

    #[test]
    fn deserialize() {
        let json =
            r#"{"registry":"registry.fly.io","username":"testuser","token":"secret123"}"#;
        let token: RegistryToken =
            serde_json::from_str(json).expect("deserialize registry token");

        assert_eq!(token.registry, "registry.fly.io");
        assert_eq!(token.username, "testuser");
        assert_eq!(token.token, "secret123");
    }

    #[test]
    fn debug() {
        let json = r#"{"registry":"registry.fly.io","username":"user","token":"tok"}"#;
        let token: RegistryToken =
            serde_json::from_str(json).expect("deserialize registry token");

        let debug_str = format!("{:?}", token);
        assert!(debug_str.contains("RegistryToken"));
    }
}

mod deploy_response_tests {
    use systemprompt_sync::api_client::DeployResponse;

    #[test]
    fn deserialize() {
        let json = r#"{"status":"success","app_url":"https://myapp.fly.dev"}"#;
        let response: DeployResponse =
            serde_json::from_str(json).expect("deserialize deploy response");

        assert_eq!(response.status, "success");
        assert_eq!(response.app_url, Some("https://myapp.fly.dev".to_string()));
    }

    #[test]
    fn no_url() {
        let json = r#"{"status":"pending","app_url":null}"#;
        let response: DeployResponse =
            serde_json::from_str(json).expect("deserialize deploy response");

        assert_eq!(response.status, "pending");
        assert!(response.app_url.is_none());
    }

    #[test]
    fn debug() {
        let json = r#"{"status":"deployed","app_url":"https://app.fly.dev"}"#;
        let response: DeployResponse =
            serde_json::from_str(json).expect("deserialize deploy response");

        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("DeployResponse"));
    }
}

mod sync_service_tests {
    use super::*;
    use systemprompt_sync::SyncService;

    #[test]
    fn creation() {
        let config =
            SyncConfig::builder(TenantId::new("tenant"), "https://api.com", "token", "/services").build();

        let service = SyncService::new(config);
        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("SyncService"));
    }

    #[test]
    fn with_full_config() {
        let config =
            SyncConfig::builder(TenantId::new("tenant"), "https://api.com", "token", "/services")
                .with_direction(SyncDirection::Pull)
                .with_dry_run(true)
                .with_verbose(true)
                .with_hostname(Some("host.com".to_string()))
                .with_sync_token(Some("sync-tok".to_string()))
                .with_local_database_url("postgresql://db:5432/app")
                .build();

        let service = SyncService::new(config);
        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("SyncService"));
    }
}

mod database_sync_service_tests {
    use super::*;
    use systemprompt_sync::DatabaseSyncService;

    #[test]
    fn creation() {
        let service = DatabaseSyncService::new(
            SyncDirection::Push,
            false,
            "postgresql://local:5432/db",
            "postgresql://cloud:5432/db",
        );

        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("DatabaseSyncService"));
    }

    #[test]
    fn dry_run() {
        let service = DatabaseSyncService::new(
            SyncDirection::Pull,
            true,
            "postgresql://local:5432/db",
            "postgresql://cloud:5432/db",
        );

        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("dry_run: true"));
    }

    #[test]
    fn direction_push() {
        let service = DatabaseSyncService::new(
            SyncDirection::Push,
            false,
            "postgresql://local:5432/db",
            "postgresql://cloud:5432/db",
        );

        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("Push"));
    }

    #[test]
    fn direction_pull() {
        let service = DatabaseSyncService::new(
            SyncDirection::Pull,
            false,
            "postgresql://local:5432/db",
            "postgresql://cloud:5432/db",
        );

        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("Pull"));
    }
}

mod file_sync_service_tests {
    use super::*;
    use systemprompt_sync::{FileSyncService, SyncApiClient};

    #[test]
    fn creation() {
        let config =
            SyncConfig::builder(TenantId::new("tenant"), "https://api.com", "token", "/services").build();

        let api_client =
            SyncApiClient::new("https://api.com", "token").expect("test client");
        let service = FileSyncService::new(config, api_client);

        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("FileSyncService"));
    }
}
