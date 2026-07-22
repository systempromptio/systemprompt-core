//! Filesystem tests for `web sitemap generate` — URL collection from the
//! content config, dynamic-route gating, base-URL resolution, and XML output.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_cli::web::sitemap::generate::{GenerateArgs, execute_with_profile};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

const CONTENT_YAML: &str = r#"
content_sources:
  docs:
    path: content/docs
    source_id: docs
    category_id: docs
    enabled: true
    sitemap:
      enabled: true
      url_pattern: /docs
      priority: 0.5
      changefreq: weekly
      parent_route:
        enabled: true
        url: /
        priority: 1.0
        changefreq: daily
  blog:
    path: content/blog
    source_id: blog
    category_id: blog
    enabled: true
    sitemap:
      enabled: true
      url_pattern: /blog/{slug}
      priority: 0.8
      changefreq: daily
  disabled_source:
    path: content/off
    source_id: off
    category_id: off
    enabled: false
    sitemap:
      enabled: true
      url_pattern: /off
      priority: 0.9
      changefreq: daily
  sitemap_off:
    path: content/quiet
    source_id: quiet
    category_id: quiet
    enabled: true
    sitemap:
      enabled: false
      url_pattern: /quiet
      priority: 0.9
      changefreq: daily
  no_sitemap:
    path: content/none
    source_id: none
    category_id: none
    enabled: true
"#;

fn make_profile(services: &Path, web_path: &Path) -> Profile {
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
            web_path: Some(web_path.to_string_lossy().to_string()),
            storage: None,
            geoip_database: None,
        },
        security: SecurityConfig {
            issuer: "https://issuer.test".to_string(),
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

struct Fixture {
    _root: tempfile::TempDir,
    profile: Profile,
    web: PathBuf,
}

fn fixture(content_yaml: &str) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let services = root.path().join("services");
    let web = root.path().join("web");
    fs::create_dir_all(services.join("content")).unwrap();
    fs::create_dir_all(services.join("web")).unwrap();
    fs::write(services.join("content/config.yaml"), content_yaml).unwrap();

    let profile = make_profile(&services, &web);
    Fixture {
        _root: root,
        profile,
        web,
    }
}

#[test]
fn generate_collects_static_urls_sorted_by_priority() {
    let fx = fixture(CONTENT_YAML);
    let args = GenerateArgs {
        output: None,
        base_url: Some("https://site.test".to_owned()),
        include_dynamic: false,
    };

    execute_with_profile(&args, &fx.profile).unwrap();

    let xml = fs::read_to_string(fx.web.join("dist/sitemap.xml")).unwrap();
    assert!(xml.contains("<loc>https://site.test/</loc>"));
    assert!(xml.contains("<loc>https://site.test/docs</loc>"));
    assert!(!xml.contains("/blog/"));
    assert!(!xml.contains("/off"));
    assert!(!xml.contains("/quiet"));
    let root_pos = xml.find("https://site.test/</loc>").unwrap();
    let docs_pos = xml.find("https://site.test/docs").unwrap();
    assert!(root_pos < docs_pos);
}

#[test]
fn generate_reads_base_url_from_metadata_and_honours_output_override() {
    let fx = fixture(CONTENT_YAML);
    fs::write(
        Path::new(&fx.profile.paths.services).join("web/metadata.yaml"),
        "baseUrl: \"https://meta.test\"\n",
    )
    .unwrap();
    let out = fx._root.path().join("custom/sitemap.xml");
    let args = GenerateArgs {
        output: Some(out.clone()),
        base_url: None,
        include_dynamic: false,
    };

    execute_with_profile(&args, &fx.profile).unwrap();

    let xml = fs::read_to_string(&out).unwrap();
    assert!(xml.contains("<loc>https://meta.test/docs</loc>"));
}

#[test]
fn generate_falls_back_to_example_base_url_without_metadata() {
    let fx = fixture(CONTENT_YAML);
    let args = GenerateArgs {
        output: None,
        base_url: None,
        include_dynamic: false,
    };

    execute_with_profile(&args, &fx.profile).unwrap();

    let xml = fs::read_to_string(fx.web.join("dist/sitemap.xml")).unwrap();
    assert!(xml.contains("<loc>https://example.com/docs</loc>"));
}

#[test]
fn generate_errors_when_content_config_missing_or_invalid() {
    let fx = fixture(CONTENT_YAML);
    fs::remove_file(Path::new(&fx.profile.paths.services).join("content/config.yaml")).unwrap();
    let args = GenerateArgs {
        output: None,
        base_url: Some("https://site.test".to_owned()),
        include_dynamic: false,
    };
    let err = execute_with_profile(&args, &fx.profile).unwrap_err();
    assert!(err.to_string().contains("Failed to read content config"));

    let fx = fixture("content_sources: [not, a, map]");
    let err = execute_with_profile(&args, &fx.profile).unwrap_err();
    assert!(err.to_string().contains("Failed to parse content config"));
}
