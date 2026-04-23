use std::path::{Path, PathBuf};

use systemprompt_models::{
    CloudConfig, CloudValidationMode, ContentNegotiationConfig, Environment, ExtensionsConfig,
    LogLevel, OutputFormat, PathsConfig, Profile, ProfileDatabaseConfig, ProfileStyle, ProfileType,
    RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig, ServerConfig, SiteConfig,
};
use systemprompt_models::profile::{expand_home, resolve_path, resolve_with_home};

fn make_paths_config(base: &str) -> PathsConfig {
    PathsConfig {
        system: format!("{base}/system"),
        services: format!("{base}/services"),
        bin: format!("{base}/bin"),
        web_path: None,
        storage: None,
        geoip_database: None,
    }
}

fn make_server_config() -> ServerConfig {
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
    }
}

fn make_security_config() -> SecurityConfig {
    SecurityConfig {
        issuer: "test-issuer".to_string(),
        access_token_expiration: 3600,
        refresh_token_expiration: 86400,
        audiences: vec![],
        allow_registration: true,
    }
}

fn make_profile(name: &str) -> Profile {
    Profile {
        name: name.to_string(),
        display_name: format!("Test {name}"),
        target: ProfileType::Local,
        site: SiteConfig {
            name: "Test Site".to_string(),
            github_link: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_string(),
            external_db_access: false,
        },
        server: make_server_config(),
        paths: make_paths_config("/tmp/test"),
        security: make_security_config(),
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        gateway: None,
    }
}

#[test]
fn resolve_path_returns_absolute_unchanged() {
    let result = resolve_path(Path::new("/base"), "/absolute/path");
    assert_eq!(result, "/absolute/path");
}

#[test]
fn resolve_path_resolves_relative_against_base() {
    let result = resolve_path(Path::new("/base/dir"), "relative/path");
    assert!(result.contains("relative/path"));
    assert!(result.starts_with('/'));
}

#[test]
fn resolve_path_resolves_dot_relative() {
    let result = resolve_path(Path::new("/base/dir"), "./subdir");
    assert!(result.contains("subdir"));
}

#[test]
fn resolve_path_resolves_parent_relative() {
    let result = resolve_path(Path::new("/base/dir"), "../sibling");
    assert!(result.contains("sibling"));
}

#[test]
fn resolve_path_with_empty_relative() {
    let result = resolve_path(Path::new("/base"), "");
    assert!(result.contains("base"));
}

#[test]
fn expand_home_with_tilde_prefix() {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let result = expand_home("~/documents/file.txt");
    let expected = PathBuf::from(home).join("documents/file.txt");
    assert_eq!(result, expected);
}

#[test]
fn expand_home_without_tilde() {
    let result = expand_home("/absolute/path");
    assert_eq!(result, PathBuf::from("/absolute/path"));
}

#[test]
fn expand_home_relative_without_tilde() {
    let result = expand_home("relative/path");
    assert_eq!(result, PathBuf::from("relative/path"));
}

#[test]
fn expand_home_just_tilde() {
    let result = expand_home("~");
    assert_eq!(result, PathBuf::from("~"));
}

#[test]
fn expand_home_tilde_no_slash() {
    let result = expand_home("~other");
    assert_eq!(result, PathBuf::from("~other"));
}

#[test]
fn resolve_with_home_absolute_path() {
    let result = resolve_with_home(Path::new("/base"), "/absolute/path");
    assert_eq!(result, PathBuf::from("/absolute/path"));
}

#[test]
fn resolve_with_home_tilde_path() {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let result = resolve_with_home(Path::new("/base"), "~/docs");
    let expected = PathBuf::from(home).join("docs");
    assert_eq!(result, expected);
}

#[test]
fn resolve_with_home_relative_path() {
    let result = resolve_with_home(Path::new("/base"), "relative/path");
    assert_eq!(result, PathBuf::from("/base/relative/path"));
}

#[test]
fn mask_secret_empty_value() {
    assert_eq!(Profile::mask_secret("", 4), "(not set)");
}

#[test]
fn mask_secret_shorter_than_visible() {
    assert_eq!(Profile::mask_secret("abc", 4), "***");
}

#[test]
fn mask_secret_equal_to_visible() {
    assert_eq!(Profile::mask_secret("abcd", 4), "***");
}

#[test]
fn mask_secret_longer_than_visible() {
    assert_eq!(Profile::mask_secret("abcdefgh", 4), "abcd...");
}

#[test]
fn mask_secret_visible_zero() {
    assert_eq!(Profile::mask_secret("any", 0), "...");
}

#[test]
fn mask_secret_visible_one() {
    assert_eq!(Profile::mask_secret("ab", 1), "a...");
}

#[test]
fn mask_secret_single_char_visible_one() {
    assert_eq!(Profile::mask_secret("x", 1), "***");
}

#[test]
fn mask_database_url_with_password() {
    let result = Profile::mask_database_url("postgres://user:secret@localhost:5432/db");
    assert!(result.contains("***"));
    assert!(result.contains("@localhost:5432/db"));
    assert!(!result.contains("secret"));
}

#[test]
fn mask_database_url_without_at_sign() {
    let url = "sqlite:///path/to/db.sqlite";
    assert_eq!(Profile::mask_database_url(url), url);
}

#[test]
fn mask_database_url_at_sign_but_no_colon() {
    let url = "user@host/db";
    assert_eq!(Profile::mask_database_url(url), url);
}

#[test]
fn is_masked_database_url_with_triple_star() {
    assert!(Profile::is_masked_database_url("postgres://user:***@localhost"));
}

#[test]
fn is_masked_database_url_with_eight_stars() {
    assert!(Profile::is_masked_database_url("postgres://user:********@localhost"));
}

#[test]
fn is_masked_database_url_plain_url() {
    assert!(!Profile::is_masked_database_url("postgres://user:pass@localhost"));
}

#[test]
fn profile_style_dev() {
    let profile = make_profile("dev");
    assert_eq!(profile.profile_style(), ProfileStyle::Development);
}

#[test]
fn profile_style_development() {
    let profile = make_profile("development");
    assert_eq!(profile.profile_style(), ProfileStyle::Development);
}

#[test]
fn profile_style_local() {
    let profile = make_profile("local");
    assert_eq!(profile.profile_style(), ProfileStyle::Development);
}

#[test]
fn profile_style_prod() {
    let profile = make_profile("prod");
    assert_eq!(profile.profile_style(), ProfileStyle::Production);
}

#[test]
fn profile_style_production() {
    let profile = make_profile("production");
    assert_eq!(profile.profile_style(), ProfileStyle::Production);
}

#[test]
fn profile_style_staging() {
    let profile = make_profile("staging");
    assert_eq!(profile.profile_style(), ProfileStyle::Staging);
}

#[test]
fn profile_style_stage() {
    let profile = make_profile("stage");
    assert_eq!(profile.profile_style(), ProfileStyle::Staging);
}

#[test]
fn profile_style_test() {
    let profile = make_profile("test");
    assert_eq!(profile.profile_style(), ProfileStyle::Test);
}

#[test]
fn profile_style_testing() {
    let profile = make_profile("testing");
    assert_eq!(profile.profile_style(), ProfileStyle::Test);
}

#[test]
fn profile_style_custom() {
    let profile = make_profile("my-custom-profile");
    assert_eq!(profile.profile_style(), ProfileStyle::Custom);
}

#[test]
fn profile_style_case_insensitive() {
    let profile = make_profile("PROD");
    assert_eq!(profile.profile_style(), ProfileStyle::Production);
}

#[test]
fn profile_type_local_is_local() {
    assert!(ProfileType::Local.is_local());
    assert!(!ProfileType::Local.is_cloud());
}

#[test]
fn profile_type_cloud_is_cloud() {
    assert!(ProfileType::Cloud.is_cloud());
    assert!(!ProfileType::Cloud.is_local());
}

#[test]
fn profile_type_default_is_local() {
    assert_eq!(ProfileType::default(), ProfileType::Local);
}

#[test]
fn profile_type_serde_roundtrip() {
    let local = ProfileType::Local;
    let json = serde_json::to_string(&local).unwrap();
    let deserialized: ProfileType = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, local);
}

#[test]
fn profile_type_cloud_serde_roundtrip() {
    let cloud = ProfileType::Cloud;
    let json = serde_json::to_string(&cloud).unwrap();
    let deserialized: ProfileType = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, cloud);
}

#[test]
fn profile_to_yaml_roundtrip() {
    let profile = make_profile("test");
    let yaml = profile.to_yaml().unwrap();
    assert!(yaml.contains("name: test"));
    assert!(yaml.contains("display_name:"));
}

#[test]
fn paths_config_skills() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.skills(), "/app/services/skills");
}

#[test]
fn paths_config_config() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.config(), "/app/services/config/config.yaml");
}

#[test]
fn paths_config_ai_config() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.ai_config(), "/app/services/ai/config.yaml");
}

#[test]
fn paths_config_content_config() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.content_config(), "/app/services/content/config.yaml");
}

#[test]
fn paths_config_web_config() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.web_config(), "/app/services/web/config.yaml");
}

#[test]
fn paths_config_web_metadata() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.web_metadata(), "/app/services/web/metadata.yaml");
}

#[test]
fn paths_config_plugins() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.plugins(), "/app/services/plugins");
}

#[test]
fn paths_config_hooks() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.hooks(), "/app/services/hooks");
}

#[test]
fn paths_config_agents() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.agents(), "/app/services/agents");
}

#[test]
fn paths_config_web_path_resolved_default() {
    let paths = make_paths_config("/app");
    assert_eq!(paths.web_path_resolved(), "/app/system/web");
}

#[test]
fn paths_config_web_path_resolved_custom() {
    let mut paths = make_paths_config("/app");
    paths.web_path = Some("/custom/web".to_string());
    assert_eq!(paths.web_path_resolved(), "/custom/web");
}

#[test]
fn paths_config_storage_resolved_none() {
    let paths = make_paths_config("/app");
    assert!(paths.storage_resolved().is_none());
}

#[test]
fn paths_config_storage_resolved_some() {
    let mut paths = make_paths_config("/app");
    paths.storage = Some("/data/storage".to_string());
    assert_eq!(paths.storage_resolved(), Some("/data/storage"));
}

#[test]
fn paths_config_geoip_resolved_none() {
    let paths = make_paths_config("/app");
    assert!(paths.geoip_database_resolved().is_none());
}

#[test]
fn paths_config_geoip_resolved_some() {
    let mut paths = make_paths_config("/app");
    paths.geoip_database = Some("/data/geoip.mmdb".to_string());
    assert_eq!(paths.geoip_database_resolved(), Some("/data/geoip.mmdb"));
}

#[test]
fn paths_config_resolve_relative_to() {
    let mut paths = PathsConfig {
        system: "system".to_string(),
        services: "services".to_string(),
        bin: "bin".to_string(),
        web_path: Some("web".to_string()),
        storage: Some("storage".to_string()),
        geoip_database: Some("geoip.mmdb".to_string()),
    };
    paths.resolve_relative_to(Path::new("/base"));
    assert!(paths.system.starts_with('/'));
    assert!(paths.services.starts_with('/'));
    assert!(paths.bin.starts_with('/'));
    assert!(paths.web_path.as_ref().unwrap().starts_with('/'));
    assert!(paths.storage.as_ref().unwrap().starts_with('/'));
    assert!(paths.geoip_database.as_ref().unwrap().starts_with('/'));
}

#[test]
fn paths_config_resolve_relative_to_preserves_absolute() {
    let mut paths = PathsConfig {
        system: "/absolute/system".to_string(),
        services: "/absolute/services".to_string(),
        bin: "/absolute/bin".to_string(),
        web_path: None,
        storage: None,
        geoip_database: None,
    };
    paths.resolve_relative_to(Path::new("/base"));
    assert_eq!(paths.system, "/absolute/system");
    assert_eq!(paths.services, "/absolute/services");
    assert_eq!(paths.bin, "/absolute/bin");
}

#[test]
fn environment_default_is_development() {
    assert_eq!(Environment::default(), Environment::Development);
}

#[test]
fn environment_display() {
    assert_eq!(Environment::Development.to_string(), "development");
    assert_eq!(Environment::Production.to_string(), "production");
    assert_eq!(Environment::Test.to_string(), "test");
    assert_eq!(Environment::Staging.to_string(), "staging");
}

#[test]
fn environment_from_str() {
    assert_eq!("development".parse::<Environment>().unwrap(), Environment::Development);
    assert_eq!("production".parse::<Environment>().unwrap(), Environment::Production);
    assert_eq!("test".parse::<Environment>().unwrap(), Environment::Test);
    assert_eq!("staging".parse::<Environment>().unwrap(), Environment::Staging);
}

#[test]
fn environment_from_str_invalid() {
    assert!("invalid".parse::<Environment>().is_err());
}

#[test]
fn environment_serde_roundtrip() {
    let env = Environment::Staging;
    let json = serde_json::to_string(&env).unwrap();
    let deserialized: Environment = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, env);
}

#[test]
fn log_level_default_is_normal() {
    assert_eq!(LogLevel::default(), LogLevel::Normal);
}

#[test]
fn log_level_as_tracing_filter() {
    assert_eq!(LogLevel::Quiet.as_tracing_filter(), "error");
    assert_eq!(LogLevel::Normal.as_tracing_filter(), "info");
    assert_eq!(LogLevel::Verbose.as_tracing_filter(), "debug");
    assert_eq!(LogLevel::Debug.as_tracing_filter(), "trace");
}

#[test]
fn log_level_display() {
    assert_eq!(LogLevel::Quiet.to_string(), "quiet");
    assert_eq!(LogLevel::Normal.to_string(), "normal");
    assert_eq!(LogLevel::Verbose.to_string(), "verbose");
    assert_eq!(LogLevel::Debug.to_string(), "debug");
}

#[test]
fn log_level_from_str() {
    assert_eq!("quiet".parse::<LogLevel>().unwrap(), LogLevel::Quiet);
    assert_eq!("normal".parse::<LogLevel>().unwrap(), LogLevel::Normal);
    assert_eq!("verbose".parse::<LogLevel>().unwrap(), LogLevel::Verbose);
    assert_eq!("debug".parse::<LogLevel>().unwrap(), LogLevel::Debug);
}

#[test]
fn log_level_from_str_invalid() {
    assert!("trace".parse::<LogLevel>().is_err());
}

#[test]
fn output_format_default_is_text() {
    assert_eq!(OutputFormat::default(), OutputFormat::Text);
}

#[test]
fn output_format_display() {
    assert_eq!(OutputFormat::Text.to_string(), "text");
    assert_eq!(OutputFormat::Json.to_string(), "json");
    assert_eq!(OutputFormat::Yaml.to_string(), "yaml");
}

#[test]
fn output_format_from_str() {
    assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
    assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    assert_eq!("yaml".parse::<OutputFormat>().unwrap(), OutputFormat::Yaml);
}

#[test]
fn output_format_from_str_invalid() {
    assert!("xml".parse::<OutputFormat>().is_err());
}

#[test]
fn extensions_config_is_disabled() {
    let config = ExtensionsConfig {
        disabled: vec!["ext-a".to_string(), "ext-b".to_string()],
    };
    assert!(config.is_disabled("ext-a"));
    assert!(config.is_disabled("ext-b"));
    assert!(!config.is_disabled("ext-c"));
}

#[test]
fn extensions_config_empty_disabled() {
    let config = ExtensionsConfig::default();
    assert!(!config.is_disabled("anything"));
}

#[test]
fn profile_style_label() {
    assert_eq!(ProfileStyle::Development.label(), "Dev");
    assert_eq!(ProfileStyle::Production.label(), "Prod");
    assert_eq!(ProfileStyle::Staging.label(), "Stage");
    assert_eq!(ProfileStyle::Test.label(), "Test");
    assert_eq!(ProfileStyle::Custom.label(), "Custom");
}

#[test]
fn cloud_validation_mode_default_is_strict() {
    assert_eq!(CloudValidationMode::default(), CloudValidationMode::Strict);
}

#[test]
fn cloud_validation_mode_serde_roundtrip() {
    for mode in [CloudValidationMode::Strict, CloudValidationMode::Warn, CloudValidationMode::Skip] {
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: CloudValidationMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, mode);
    }
}

#[test]
fn cloud_config_default() {
    let config = CloudConfig::default();
    assert!(config.tenant_id.is_none());
    assert_eq!(config.validation, CloudValidationMode::Strict);
}

#[test]
fn runtime_config_default() {
    let config = RuntimeConfig::default();
    assert_eq!(config.environment, Environment::Development);
    assert_eq!(config.log_level, LogLevel::Normal);
    assert_eq!(config.output_format, OutputFormat::Text);
    assert!(!config.no_color);
    assert!(!config.non_interactive);
}

#[test]
fn rate_limits_config_default_values() {
    let config = RateLimitsConfig::default();
    assert!(!config.disabled);
    assert_eq!(config.oauth_public_per_second, 10);
    assert_eq!(config.oauth_auth_per_second, 10);
    assert_eq!(config.contexts_per_second, 100);
    assert_eq!(config.tasks_per_second, 50);
    assert_eq!(config.burst_multiplier, 3);
}

#[test]
fn rate_limits_tier_multipliers_default() {
    let config = RateLimitsConfig::default();
    let tiers = config.tier_multipliers;
    assert!((tiers.admin - 10.0).abs() < f64::EPSILON);
    assert!((tiers.user - 1.0).abs() < f64::EPSILON);
    assert!((tiers.anon - 0.5).abs() < f64::EPSILON);
}

#[test]
fn content_negotiation_default() {
    let config = ContentNegotiationConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.markdown_suffix, ".md");
}

#[test]
fn security_headers_default() {
    let config = SecurityHeadersConfig::default();
    assert!(config.enabled);
    assert_eq!(config.frame_options, "DENY");
    assert_eq!(config.content_type_options, "nosniff");
    assert!(config.content_security_policy.is_none());
}

#[test]
fn database_config_serde_roundtrip() {
    let config = ProfileDatabaseConfig {
        db_type: "postgres".to_string(),
        external_db_access: true,
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: ProfileDatabaseConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.db_type, "postgres");
    assert!(deserialized.external_db_access);
}

#[test]
fn site_config_serde_roundtrip() {
    let config = SiteConfig {
        name: "My Site".to_string(),
        github_link: Some("https://github.com/example".to_string()),
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: SiteConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "My Site");
    assert_eq!(deserialized.github_link.as_deref(), Some("https://github.com/example"));
}

#[test]
fn site_config_github_link_none() {
    let config = SiteConfig {
        name: "Test".to_string(),
        github_link: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: SiteConfig = serde_json::from_str(&json).unwrap();
    assert!(deserialized.github_link.is_none());
}

#[test]
fn profile_field_access() {
    let profile = make_profile("myprofile");
    assert_eq!(profile.name, "myprofile");
    assert_eq!(profile.display_name, "Test myprofile");
    assert_eq!(profile.server.host, "127.0.0.1");
    assert_eq!(profile.server.port, 8080);
    assert_eq!(profile.database.db_type, "postgres");
    assert!(!profile.database.external_db_access);
}
