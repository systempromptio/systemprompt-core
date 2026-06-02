use systemprompt_models::profile::default_resource_audiences;
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

fn server_config() -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".to_string(),
        port: 8080,
        api_server_url: "http://localhost:8080".to_string(),
        api_internal_url: "http://localhost:8080".to_string(),
        api_external_url: "https://example.com".to_string(),
        use_https: false,
        cors_allowed_origins: vec![],
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        instance_id: None,
        max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
        trusted_proxies: Vec::new(),
    }
}

fn security_config() -> SecurityConfig {
    SecurityConfig {
        issuer: "test-issuer".to_string(),
        access_token_expiration: 3600,
        refresh_token_expiration: 86400,
        audiences: vec![],
        allowed_resource_audiences: default_resource_audiences(),
        allow_registration: true,
        signing_key_path: std::path::PathBuf::from("/tmp/test-signing-key.pem"),
        trusted_issuers: vec![],
    }
}

fn local_paths() -> PathsConfig {
    PathsConfig {
        system: "/tmp/system".to_string(),
        services: "/tmp/services".to_string(),
        bin: "/tmp/bin".to_string(),
        web_path: None,
        storage: None,
        geoip_database: None,
    }
}

fn cloud_paths() -> PathsConfig {
    PathsConfig {
        system: "/app".to_string(),
        services: "/app/services".to_string(),
        bin: "/app/bin".to_string(),
        web_path: Some("/app/web".to_string()),
        storage: None,
        geoip_database: None,
    }
}

fn valid_profile() -> Profile {
    Profile {
        name: "p".to_string(),
        display_name: "Profile P".to_string(),
        target: ProfileType::Local,
        site: SiteConfig {
            name: "Site".to_string(),
            github_link: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_string(),
            external_db_access: false,
        },
        server: server_config(),
        paths: local_paths(),
        security: security_config(),
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        providers: systemprompt_models::profile::ProviderRegistry::default(),
        gateway: None,
        governance: None,
        system_admin: SystemAdminConfig {
            username: "admin".to_string(),
        },
    }
}

fn errors_of(profile: &Profile) -> String {
    profile
        .validate()
        .err()
        .map_or_else(String::new, |e| format!("{e}"))
}

mod required_fields {
    use super::*;

    #[test]
    fn valid_local_profile_passes() {
        assert!(valid_profile().validate().is_ok());
    }

    #[test]
    fn empty_name_is_rejected() {
        let mut p = valid_profile();
        p.name = String::new();
        assert!(errors_of(&p).contains("Profile name"));
    }

    #[test]
    fn empty_display_name_is_rejected() {
        let mut p = valid_profile();
        p.display_name = String::new();
        assert!(errors_of(&p).contains("display_name"));
    }

    #[test]
    fn empty_site_name_is_rejected() {
        let mut p = valid_profile();
        p.site.name = String::new();
        assert!(errors_of(&p).contains("Site name"));
    }

    #[test]
    fn empty_host_is_rejected() {
        let mut p = valid_profile();
        p.server.host = String::new();
        assert!(errors_of(&p).contains("Server host"));
    }

    #[test]
    fn empty_api_urls_rejected() {
        let mut p = valid_profile();
        p.server.api_server_url = String::new();
        p.server.api_internal_url = String::new();
        p.server.api_external_url = String::new();
        let msg = errors_of(&p);
        assert!(msg.contains("api_server_url"));
        assert!(msg.contains("api_internal_url"));
        assert!(msg.contains("api_external_url"));
    }

    #[test]
    fn zero_port_is_rejected() {
        let mut p = valid_profile();
        p.server.port = 0;
        assert!(errors_of(&p).contains("port must be greater than 0"));
    }
}

mod security_settings {
    use super::*;

    #[test]
    fn nonpositive_access_token_expiration_rejected() {
        let mut p = valid_profile();
        p.security.access_token_expiration = 0;
        assert!(errors_of(&p).contains("access_token_expiration must be positive"));
    }

    #[test]
    fn negative_refresh_token_expiration_rejected() {
        let mut p = valid_profile();
        p.security.refresh_token_expiration = -1;
        assert!(errors_of(&p).contains("refresh_token_expiration must be positive"));
    }

    #[test]
    fn missing_hook_audience_rejected() {
        let mut p = valid_profile();
        p.security.allowed_resource_audiences = vec![];
        let msg = errors_of(&p);
        assert!(msg.contains("allowed_resource_audiences"));
        assert!(msg.contains("\"hook\""));
    }
}

mod cors_origins {
    use super::*;

    #[test]
    fn empty_origin_rejected() {
        let mut p = valid_profile();
        p.server.cors_allowed_origins = vec![String::new()];
        assert!(errors_of(&p).contains("CORS origin cannot be empty"));
    }

    #[test]
    fn wildcard_origin_rejected() {
        let mut p = valid_profile();
        p.server.cors_allowed_origins = vec!["*".to_string()];
        assert!(errors_of(&p).contains("'*' is not permitted"));
    }

    #[test]
    fn plain_http_remote_origin_rejected() {
        let mut p = valid_profile();
        p.server.cors_allowed_origins = vec!["http://evil.example.com".to_string()];
        assert!(errors_of(&p).contains("Invalid CORS origin"));
    }

    #[test]
    fn https_origin_accepted() {
        let mut p = valid_profile();
        p.server.cors_allowed_origins = vec!["https://app.example.com".to_string()];
        assert!(!errors_of(&p).contains("CORS"));
    }

    #[test]
    fn loopback_http_origins_accepted() {
        let mut p = valid_profile();
        p.server.cors_allowed_origins = vec![
            "http://localhost:3000".to_string(),
            "http://127.0.0.1:3000".to_string(),
            "http://[::1]:3000".to_string(),
        ];
        assert!(!errors_of(&p).contains("CORS"));
    }
}

mod rate_limits {
    use super::*;

    #[test]
    fn disabled_rate_limits_skip_validation() {
        let mut p = valid_profile();
        p.rate_limits.disabled = true;
        p.rate_limits.burst_multiplier = 0;
        p.rate_limits.contexts_per_second = 0;
        assert!(!errors_of(&p).contains("rate_limits"));
    }

    #[test]
    fn zero_burst_multiplier_rejected() {
        let mut p = valid_profile();
        p.rate_limits.disabled = false;
        p.rate_limits.burst_multiplier = 0;
        assert!(errors_of(&p).contains("burst_multiplier must be greater than 0"));
    }

    #[test]
    fn zero_contexts_rate_rejected() {
        let mut p = valid_profile();
        p.rate_limits.disabled = false;
        p.rate_limits.contexts_per_second = 0;
        assert!(errors_of(&p).contains("contexts_per_second must be greater than 0"));
    }

    #[test]
    fn zero_stream_rate_rejected() {
        let mut p = valid_profile();
        p.rate_limits.disabled = false;
        p.rate_limits.stream_per_second = 0;
        assert!(errors_of(&p).contains("stream_per_second must be greater than 0"));
    }
}

mod cloud_paths_validation {
    use super::*;

    fn cloud_profile() -> Profile {
        let mut p = valid_profile();
        p.target = ProfileType::Cloud;
        p.paths = cloud_paths();
        p
    }

    #[test]
    fn valid_cloud_profile_passes() {
        assert!(cloud_profile().validate().is_ok());
    }

    #[test]
    fn non_app_system_path_rejected() {
        let mut p = cloud_profile();
        p.paths.system = "/var/system".to_string();
        assert!(errors_of(&p).contains("should start with /app"));
    }

    #[test]
    fn web_path_not_under_app_web_rejected() {
        let mut p = cloud_profile();
        p.paths.web_path = Some("/app/other".to_string());
        assert!(errors_of(&p).contains("/app/web"));
    }

    #[test]
    fn web_path_pointing_at_services_web_rejected() {
        let mut p = cloud_profile();
        p.paths.web_path = Some("/app/services/web".to_string());
        assert!(errors_of(&p).contains("/app/services/web"));
    }

    #[test]
    fn empty_cloud_system_path_rejected() {
        let mut p = cloud_profile();
        p.paths.system = String::new();
        assert!(errors_of(&p).contains("Paths system"));
    }
}
