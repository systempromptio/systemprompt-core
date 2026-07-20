//! Shared profile + FilesConfig bootstrap for the files integration tests.
//!
//! `ProfileBootstrap` and `FilesConfig` are process-wide `OnceLock` globals,
//! so every test in this binary cooperates on one tempdir-backed profile and
//! exactly one initialisation. Returns the [`TestEnv`] so tests can reach the
//! tempdir paths without re-creating them.

use std::path::PathBuf;
use std::sync::OnceLock;

use systemprompt_config::ProfileBootstrap;
use systemprompt_files::FilesConfig;
use systemprompt_models::AppPaths;
use tempfile::TempDir;

pub struct TestEnv {
    pub _tmp: TempDir,
    pub storage_root: PathBuf,
    pub app_paths: AppPaths,
}

static ENV: OnceLock<TestEnv> = OnceLock::new();

pub fn test_env() -> &'static TestEnv {
    ENV.get_or_init(init_env)
}

fn init_env() -> TestEnv {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let tmp_path = tmp.path().to_path_buf();

    let system_dir = tmp_path.join("system");
    let services_dir = tmp_path.join("services");
    let bin_dir = tmp_path.join("bin");
    let storage_dir = tmp_path.join("storage");
    for dir in [&system_dir, &services_dir, &bin_dir, &storage_dir] {
        std::fs::create_dir_all(dir).expect("mkdir test path");
    }

    let yaml = format!(
        r#"
name: test
display_name: Integration Test Profile
target: local
site:
  name: testsite
  github_link: null
database:
  type: postgres
  external_db_access: false
server:
  host: localhost
  port: 8080
  api_server_url: http://localhost:8080
  api_internal_url: http://localhost:8080
  api_external_url: http://localhost:8080
  use_https: false
  cors_allowed_origins:
    - http://localhost:8080
  content_negotiation:
    enabled: false
    markdown_suffix: .md
  security_headers:
    enabled: true
    hsts: max-age=63072000; includeSubDomains; preload
    frame_options: DENY
    content_type_options: nosniff
    referrer_policy: strict-origin-when-cross-origin
    permissions_policy: camera=()
    content_security_policy: null
  instance_id: null
  max_concurrent_streams: 256
  trusted_proxies: []
paths:
  system: {system}
  services: {services}
  bin: {bin}
  web_path: null
  storage: {storage}
  geoip_database: null
security:
  jwt_issuer: https://issuer.test
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
  jwt_audiences:
    - api
  allowed_resource_audiences:
    - hook
  allow_registration: true
  signing_key_path: signing_key.pem
rate_limits:
  disabled: true
  oauth_public_per_second: 10
  oauth_auth_per_second: 10
  contexts_per_second: 100
  tasks_per_second: 50
  artifacts_per_second: 50
  agent_registry_per_second: 50
  agents_per_second: 20
  mcp_registry_per_second: 50
  mcp_per_second: 200
  stream_per_second: 100
  content_per_second: 50
  burst_multiplier: 3
  tier_multipliers:
    admin: 10.0
    user: 1.0
    a2a: 5.0
    mcp: 5.0
    service: 5.0
    anon: 0.5
system_admin:
  username: testadmin
runtime:
  environment: development
  log_level: quiet
  output_format: text
  no_color: true
  non_interactive: true
extensions:
  disabled: []
"#,
        system = system_dir.display(),
        services = services_dir.display(),
        bin = bin_dir.display(),
        storage = storage_dir.display(),
    );

    let profile_path = tmp_path.join("profile.yaml");
    std::fs::write(&profile_path, yaml).expect("write profile yaml");

    if let Err(e) = ProfileBootstrap::init_from_path(&profile_path) {
        eprintln!("ProfileBootstrap::init_from_path failed: {e}");
    }

    let profile = ProfileBootstrap::get().expect("profile initialised");
    let app_paths = AppPaths::from_profile(&profile.paths).expect("app paths");

    if let Err(e) = FilesConfig::init(&app_paths) {
        eprintln!("FilesConfig::init failed: {e}");
    }

    TestEnv {
        _tmp: tmp,
        storage_root: storage_dir,
        app_paths,
    }
}
