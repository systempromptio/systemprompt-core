use systemprompt_manifest::Config;
use systemprompt_manifest::config::{is_random_instance_id, random_instance_id};
use systemprompt_manifest::profile::{
    ContentNegotiationConfig, RateLimitsConfig, SecurityHeadersConfig,
};
use systemprompt_traits::ConfigProvider;

fn fixture_config() -> Config {
    Config {
        instance_id: systemprompt_identifiers::InstanceId::new("test"),
        metrics_port: None,
        max_concurrent_streams: 16,
        role: Default::default(),
        max_in_flight: None,
        shutdown: Default::default(),
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://localhost/x".to_string(),
        database_write_url: None,
        database_session_url: None,
        github_link: String::new(),
        github_token: None,
        system_path: "/tmp".to_string(),
        services_path: "/tmp".to_string(),
        bin_path: "/tmp".to_string(),
        skills_path: "/tmp".to_string(),
        settings_path: "/tmp".to_string(),
        content_config_path: String::new(),
        geoip_database_path: None,
        web_path: String::new(),
        web_config_path: String::new(),
        web_metadata_path: String::new(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        api_server_url: "http://127.0.0.1".to_string(),
        api_internal_url: "http://127.0.0.1".to_string(),
        api_external_url: "http://127.0.0.1".to_string(),
        jwt_issuer: "https://issuer.test".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: Vec::new(),
        allowed_resource_audiences: Vec::new(),
        trusted_issuers: Vec::new(),
        id_jag_ttl_secs: 300,
        signing_key_path: std::path::PathBuf::new(),
        use_https: false,
        rate_limits: RateLimitsConfig::default(),
        retention: systemprompt_manifest::profile::RetentionConfig::default(),
        planes: Default::default(),
        cors_allowed_origins: Vec::new(),
        trusted_proxies: Vec::new(),
        is_cloud: false,
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
        allow_dynamic_client_registration: true,
        api_key_max_lifetime_days: None,
        login_page_url: None,
        system_admin_username: "admin".to_string(),
        system_admin_email: None,
    }
}

#[test]
fn config_provider_serves_every_named_key() {
    let mut cfg = fixture_config();
    cfg.database_write_url = Some("postgres://writer/x".to_owned());
    cfg.database_session_url = Some("postgres://session/x".to_owned());
    cfg.github_token = Some("ghp_x".to_owned());
    let expect = [
        ("database_type", "postgres"),
        ("database_url", "postgres://localhost/x"),
        ("database_write_url", "postgres://writer/x"),
        ("database_session_url", "postgres://session/x"),
        ("host", "127.0.0.1"),
        ("port", "8080"),
        ("system_path", "/tmp"),
        ("services_path", "/tmp"),
        ("bin_path", "/tmp"),
        ("skills_path", "/tmp"),
        ("settings_path", "/tmp"),
        ("content_config_path", ""),
        ("web_path", ""),
        ("web_config_path", ""),
        ("web_metadata_path", ""),
        ("sitename", "test"),
        ("github_link", ""),
        ("github_token", "ghp_x"),
        ("api_server_url", "http://127.0.0.1"),
        ("api_external_url", "http://127.0.0.1"),
        ("jwt_issuer", "https://issuer.test"),
        ("is_cloud", "false"),
        ("instance_id", "test"),
        ("max_concurrent_streams", "16"),
    ];
    for (key, value) in expect {
        assert_eq!(
            ConfigProvider::get(&cfg, key).as_deref(),
            Some(value),
            "{key}"
        );
    }
    assert_eq!(
        ConfigProvider::get(&cfg, "role"),
        Some(cfg.role.to_string())
    );
    assert_eq!(ConfigProvider::get(&cfg, "no_such_key"), None);
    assert_eq!(cfg.database_url(), "postgres://localhost/x");
    assert_eq!(cfg.database_write_url(), Some("postgres://writer/x"));
    assert_eq!(cfg.system_path(), "/tmp");
    assert_eq!(cfg.api_port(), 8080);
    assert!(cfg.as_any().downcast_ref::<Config>().is_some());
    assert_eq!(cfg.logs_path(), "/tmp/logs");
}

#[test]
fn optional_keys_are_absent_when_unset() {
    let cfg = fixture_config();
    for key in ["database_write_url", "database_session_url", "github_token"] {
        assert_eq!(ConfigProvider::get(&cfg, key), None, "{key}");
    }
    assert_eq!(cfg.database_write_url(), None);
}

#[test]
fn debug_output_redacts_credentials() {
    let mut cfg = fixture_config();
    cfg.database_url = "postgres://user:hunter2@db/x".to_owned();
    cfg.github_token = Some("ghp_secret".to_owned());
    let rendered = format!("{cfg:?}");
    assert!(!rendered.contains("hunter2"), "{rendered}");
    assert!(!rendered.contains("ghp_secret"), "{rendered}");
    assert!(rendered.contains("<redacted>"), "{rendered}");
}

#[test]
fn random_instance_ids_are_recognised_and_stable_ones_are_not() {
    let id = random_instance_id();
    assert!(is_random_instance_id(id.as_str()), "{id}");
    assert_ne!(random_instance_id().as_str(), id.as_str());
    for stable in [
        "web-1",
        "instance-",
        "instance-0123",
        "instance-0123456789ABCDEF0123456789abcdef",
        "instance-0123456789abcdef0123456789abcdeg",
        "xinstance-0123456789abcdef0123456789abcdef",
    ] {
        assert!(!is_random_instance_id(stable), "{stable}");
    }
    assert!(is_random_instance_id(
        "instance-0123456789abcdef0123456789abcdef"
    ));
}
