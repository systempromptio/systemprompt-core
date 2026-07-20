//! Tests for `admin config` — the `ConfigSection` model and its YAML helpers,
//! plus the file-oriented branches of `admin config validate`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};
use std::str::FromStr;

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::config::config_section::{
    ConfigSection, read_yaml_file, write_yaml_file,
};
use systemprompt_cli::admin::config::validate::{ValidateArgs, execute};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

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

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

#[test]
fn config_section_from_str_parses_every_variant_case_insensitively() {
    let cases = [
        ("ai", ConfigSection::Ai),
        ("Content", ConfigSection::Content),
        ("WEB", ConfigSection::Web),
        ("scheduler", ConfigSection::Scheduler),
        ("Agents", ConfigSection::Agents),
        ("mcp", ConfigSection::Mcp),
        ("skills", ConfigSection::Skills),
        ("profile", ConfigSection::Profile),
        ("services", ConfigSection::Services),
    ];
    for (input, expected) in cases {
        assert_eq!(ConfigSection::from_str(input).unwrap(), expected);
    }
}

#[test]
fn config_section_from_str_rejects_unknown_section() {
    let err = ConfigSection::from_str("nope").unwrap_err();
    assert!(
        format!("{err}").contains("Unknown config section"),
        "got: {err}"
    );
}

#[test]
fn config_section_display_round_trips_through_from_str() {
    for section in ConfigSection::all() {
        let text = section.to_string();
        assert_eq!(ConfigSection::from_str(&text).unwrap(), *section);
    }
}

#[test]
fn config_section_all_lists_nine_unique_sections() {
    let all = ConfigSection::all();
    assert_eq!(all.len(), 9);
    let mut names: Vec<String> = all.iter().map(ToString::to_string).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 9);
}

#[test]
fn read_yaml_file_round_trips_a_written_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.yaml");
    let value: serde_yaml::Value =
        serde_yaml::from_str("outer:\n  inner: 7\n  keep: preserved\n").unwrap();

    write_yaml_file(&path, &value).unwrap();
    let read_back = read_yaml_file(&path).unwrap();

    assert_eq!(read_back["outer"]["inner"], serde_yaml::Value::from(7));
    assert_eq!(
        read_back["outer"]["keep"],
        serde_yaml::Value::from("preserved")
    );
}

#[test]
fn read_yaml_file_errors_on_missing_file() {
    let err = read_yaml_file(Path::new("/nonexistent/config/does-not-exist.yaml")).unwrap_err();
    assert!(
        format!("{err:#}").contains("Failed to read file"),
        "got: {err:#}"
    );
}

#[test]
fn read_yaml_file_errors_on_malformed_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.yaml");
    std::fs::write(&path, "key: [unterminated\n").unwrap();

    let err = read_yaml_file(&path).unwrap_err();
    assert!(
        format!("{err:#}").contains("Failed to parse YAML"),
        "got: {err:#}"
    );
}

#[test]
fn validate_schema_flag_reports_valid_and_skips_render() {
    let args = ValidateArgs {
        target: None,
        strict: false,
        schema: true,
    };
    let (_output, all_valid) = execute(&args, &cfg()).unwrap();
    assert!(all_valid);
}

#[test]
fn validate_accepts_a_serialized_profile_document() {
    let dir = tempfile::tempdir().unwrap();
    let services = dir.path().join("services");
    std::fs::create_dir_all(&services).unwrap();
    let yaml = make_profile(&services).to_yaml().unwrap();
    let profile_path = dir.path().join("profile.yaml");
    std::fs::write(&profile_path, yaml).unwrap();

    let args = ValidateArgs {
        target: Some(profile_path.to_string_lossy().to_string()),
        strict: false,
        schema: false,
    };
    let (_output, all_valid) = execute(&args, &cfg()).unwrap();
    assert!(all_valid);
}

#[test]
fn validate_rejects_a_malformed_profile_document() {
    let dir = tempfile::tempdir().unwrap();
    let profile_path = dir.path().join("profile.yaml");
    std::fs::write(&profile_path, "name: only-a-name\ntarget: local\n").unwrap();

    let args = ValidateArgs {
        target: Some(profile_path.to_string_lossy().to_string()),
        strict: false,
        schema: false,
    };
    let err = execute(&args, &cfg()).unwrap_err();
    assert!(
        format!("{err:#}").contains("invalid profile"),
        "got: {err:#}"
    );
}

#[test]
fn validate_reports_missing_file_and_detects_section_from_path() {
    let cases = [
        ("/no/such/ai/config.yaml", "ai"),
        ("/no/such/content/config.yaml", "content"),
        ("/no/such/web/config.yaml", "web"),
        ("/no/such/scheduler/config.yaml", "scheduler"),
        ("/no/such/agents/config.yaml", "agents"),
        ("/no/such/mcp/config.yaml", "mcp"),
        ("/no/such/skills/config.yaml", "skills"),
        ("/no/such/place/profile.yaml", "profile"),
        ("/no/such/config/config.yaml", "services"),
        ("/no/such/place/other.yaml", "unknown"),
    ];

    for (target, expected_section) in cases {
        let args = ValidateArgs {
            target: Some(target.to_string()),
            strict: false,
            schema: false,
        };
        let (output, all_valid) = execute(&args, &cfg()).unwrap();
        assert!(!all_valid, "missing file should fail: {target}");
        let json = serde_json::to_value(output.artifact()).unwrap();
        let files = json["items"].as_array().expect("items array in artifact");
        assert_eq!(files.len(), 1, "one file entry for {target}");
        assert_eq!(
            files[0]["section"], expected_section,
            "section for {target}"
        );
        assert_eq!(files[0]["exists"], false);
        assert_eq!(files[0]["error"], "File not found");
    }
}
