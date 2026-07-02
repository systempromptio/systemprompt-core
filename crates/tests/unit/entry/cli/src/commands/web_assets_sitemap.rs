//! Tests for `web assets list`, `web sitemap show`, and the web validators.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_cli::web::assets::list::{AssetTypeFilter, ListArgs, execute_in_dir};
use systemprompt_cli::web::paths::WebPaths;
use systemprompt_cli::web::sitemap::show::{ShowArgs, execute_with_config_path};
use systemprompt_cli::web::validate::asset_validation::validate_assets;
use systemprompt_cli::web::validate::sitemap_validation::validate_sitemap;
use systemprompt_cli::web::validate::template_validation::validate_templates;
use systemprompt_cli::{CliConfig, OutputFormat};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

fn cfg() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

fn make_profile(services: &Path) -> Profile {
    Profile {
        name: "test".to_string(),
        display_name: "Test".to_string(),
        target: ProfileType::Local,
        site: SiteConfig {
            name: "Test Site".to_string(),
            github_link: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_string(),
            external_db_access: false,
            pool: None,
        },
        server: ServerConfig {
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
        },
        paths: PathsConfig {
            system: services.parent().unwrap().to_string_lossy().to_string(),
            services: services.to_string_lossy().to_string(),
            bin: "/tmp/test/bin".to_string(),
            web_path: None,
            storage: None,
            geoip_database: None,
        },
        security: SecurityConfig {
            issuer: "test-issuer".to_string(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
            audiences: vec![JwtAudience::Api],
            allowed_resource_audiences: vec![],
            allow_registration: true,
            signing_key_path: PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
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

const CONTENT_CONFIG: &str = r#"
content_sources:
  docs:
    path: content/docs
    source_id: docs
    category_id: docs
    enabled: true
    sitemap:
      enabled: true
      url_pattern: "/docs/{slug}"
      priority: 0.8
      changefreq: weekly
      parent_route:
        enabled: true
        url: "/docs"
        priority: 0.9
        changefreq: daily
  drafts:
    path: content/drafts
    source_id: drafts
    category_id: drafts
    enabled: false
"#;

#[test]
fn assets_list_reports_empty_and_typed_entries() {
    let missing = execute_in_dir(
        ListArgs {
            asset_type: AssetTypeFilter::All,
        },
        &cfg(),
        Path::new("/nonexistent/assets-dir"),
    )
    .unwrap();
    let json = serde_json::to_value(missing.artifact()).unwrap();
    assert!(json["items"].as_array().unwrap().is_empty());

    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("style.css"), "body{}").unwrap();
    fs::create_dir_all(dir.path().join("img")).unwrap();
    fs::write(dir.path().join("img/logo.png"), [0u8; 4]).unwrap();

    let all = execute_in_dir(
        ListArgs {
            asset_type: AssetTypeFilter::All,
        },
        &cfg(),
        dir.path(),
    )
    .unwrap();
    let json = serde_json::to_value(all.artifact()).unwrap();
    assert_eq!(json["items"].as_array().unwrap().len(), 2);

    let css_only = execute_in_dir(
        ListArgs {
            asset_type: AssetTypeFilter::Css,
        },
        &cfg(),
        dir.path(),
    )
    .unwrap();
    let json = serde_json::to_value(css_only.artifact()).unwrap();
    let items = json["items"].as_array().unwrap();
    assert!(
        items
            .iter()
            .all(|i| i["path"].as_str().unwrap().ends_with(".css"))
    );
}

#[test]
fn sitemap_show_lists_enabled_routes() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(&config_path, CONTENT_CONFIG).unwrap();

    let out = execute_with_config_path(
        ShowArgs { preview: true },
        &cfg(),
        config_path.to_str().unwrap(),
    )
    .unwrap();
    let json = serde_json::to_value(out.artifact()).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["url"], "/docs");

    let err = execute_with_config_path(
        ShowArgs { preview: false },
        &cfg(),
        "/nonexistent/content-config.yaml",
    )
    .unwrap_err();
    assert!(err.to_string().contains("Failed to read content config"));
}

#[test]
fn validate_templates_flags_missing_files_and_unknown_types() {
    let services = tempfile::tempdir().unwrap();
    fs::create_dir_all(services.path().join("content")).unwrap();
    fs::write(services.path().join("content/config.yaml"), CONTENT_CONFIG).unwrap();

    let templates_dir = tempfile::tempdir().unwrap();
    fs::write(
        templates_dir.path().join("templates.yaml"),
        "templates:\n  article:\n    content_types: [docs, mystery]\n",
    )
    .unwrap();

    let profile = make_profile(services.path());
    let web_paths = WebPaths {
        templates: templates_dir.path().to_path_buf(),
        assets: templates_dir.path().join("assets"),
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    validate_templates(&profile, &web_paths, &mut errors, &mut warnings);

    assert!(errors.iter().any(|e| {
        e.message
            .contains("Missing HTML file for template 'article'")
    }));
    assert!(
        warnings
            .iter()
            .any(|w| w.message.contains("unknown content type 'mystery'"))
    );
}

#[test]
fn validate_templates_warns_when_config_absent() {
    let services = tempfile::tempdir().unwrap();
    let templates_dir = tempfile::tempdir().unwrap();
    let profile = make_profile(services.path());
    let web_paths = WebPaths {
        templates: templates_dir.path().to_path_buf(),
        assets: templates_dir.path().join("assets"),
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    validate_templates(&profile, &web_paths, &mut errors, &mut warnings);
    assert!(
        warnings
            .iter()
            .any(|w| w.message.contains("templates.yaml not found"))
    );
}

#[test]
fn validate_sitemap_and_assets_run_on_fixture_profile() {
    let services = tempfile::tempdir().unwrap();
    fs::create_dir_all(services.path().join("content")).unwrap();
    fs::write(services.path().join("content/config.yaml"), CONTENT_CONFIG).unwrap();
    fs::create_dir_all(services.path().join("web")).unwrap();
    fs::write(services.path().join("web/config.yaml"), "site: {}\n").unwrap();

    let assets_dir = tempfile::tempdir().unwrap();
    fs::write(assets_dir.path().join("style.css"), "body{}").unwrap();

    let profile = make_profile(services.path());
    let web_paths = WebPaths {
        templates: assets_dir.path().to_path_buf(),
        assets: assets_dir.path().to_path_buf(),
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    validate_sitemap(&profile, &mut errors, &mut warnings);
    validate_assets(&profile, &web_paths, &mut errors, &mut warnings);
}
