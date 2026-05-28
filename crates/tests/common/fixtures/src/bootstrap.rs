//! Process-wide bootstrap fixture.
//!
//! Initialises [`ProfileBootstrap`], [`SecretsBootstrap`], the runtime
//! [`Config`] singleton, and [`FilesConfig`] from a tempdir-backed
//! profile bundle.

use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

use systemprompt_config::{init_config_from_profile, ProfileBootstrap, SecretsBootstrap};
use systemprompt_files::FilesConfig;
use systemprompt_models::profile::UNRESTRICTED_ACKNOWLEDGEMENT;
use systemprompt_models::{AppPaths, Config};
use tempfile::TempDir;

const TEST_OAUTH_AT_REST_PEPPER: &str = "test_oauth_at_rest_pepper_for_bootstrap_fixture_zzz";
const TEST_MANIFEST_SIGNING_SEED: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

pub struct TestBootstrap {
    pub _tmp: TempDir,
    pub profile_path: PathBuf,
    pub system_path: PathBuf,
    pub services_path: PathBuf,
    pub bin_path: PathBuf,
    pub storage_path: PathBuf,
    pub app_paths: AppPaths,
    pub database_url: String,
}

static BOOTSTRAP: OnceLock<TestBootstrap> = OnceLock::new();

pub fn ensure_test_bootstrap() -> &'static TestBootstrap {
    BOOTSTRAP.get_or_init(init_bootstrap)
}

fn init_bootstrap() -> TestBootstrap {
    let database_url = env::var("TEST_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgres://systemprompt_admin:password@localhost:5432/systemprompt_test".to_owned()
        });

    install_subprocess_env(&database_url);

    let tmp = tempfile::tempdir().expect("create bootstrap tempdir");
    let tmp_path = tmp.path().to_path_buf();

    let system_path = tmp_path.join("system");
    let services_path = tmp_path.join("services");
    let bin_path = tmp_path.join("bin");
    let storage_path = tmp_path.join("storage");
    let web_path = system_path.join("web");

    for dir in [
        &system_path,
        &services_path,
        &bin_path,
        &storage_path,
        &web_path,
        &services_path.join("config"),
        &services_path.join("content"),
        &services_path.join("web"),
        &services_path.join("skills"),
        &services_path.join("agents"),
        &services_path.join("plugins"),
        &services_path.join("hooks"),
        &services_path.join("marketplaces"),
    ] {
        std::fs::create_dir_all(dir).expect("mkdir bootstrap path");
    }

    write_yaml_stub(&services_path.join("config/config.yaml"));
    write_yaml_stub(&services_path.join("content/config.yaml"));
    write_yaml_stub(&services_path.join("web/config.yaml"));
    write_yaml_stub(&services_path.join("web/metadata.yaml"));

    let profile_yaml = render_profile_yaml(
        &system_path,
        &services_path,
        &bin_path,
        &storage_path,
        &web_path,
    );
    let profile_path = tmp_path.join("profile.yaml");
    std::fs::write(&profile_path, profile_yaml).expect("write profile.yaml");

    if !ProfileBootstrap::is_initialized() {
        if let Err(e) = ProfileBootstrap::init_from_path(&profile_path) {
            panic!("ProfileBootstrap::init_from_path failed: {e}");
        }
    }
    let profile = ProfileBootstrap::get().expect("profile initialised");

    if !SecretsBootstrap::is_initialized() {
        let _ = SecretsBootstrap::try_init();
    }

    if !Config::is_initialized() {
        let _ = init_config_from_profile(profile);
    }

    let app_paths = AppPaths::from_profile(&profile.paths).expect("app paths");
    if FilesConfig::get_optional().is_none() {
        let _ = FilesConfig::init(&app_paths);
    }

    TestBootstrap {
        _tmp: tmp,
        profile_path,
        system_path,
        services_path,
        bin_path,
        storage_path,
        app_paths,
        database_url,
    }
}

fn install_subprocess_env(database_url: &str) {
    unsafe {
        env::set_var("SYSTEMPROMPT_SUBPROCESS", "1");
        if env::var("DATABASE_URL").is_err() {
            env::set_var("DATABASE_URL", database_url);
        }
        if env::var("OAUTH_AT_REST_PEPPER").is_err() {
            env::set_var("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER);
        }
        if env::var("MANIFEST_SIGNING_SECRET_SEED").is_err() {
            env::set_var("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED);
        }
    }
}

fn write_yaml_stub(path: &std::path::Path) {
    std::fs::write(path, "{}\n").expect("write yaml stub");
}

fn render_profile_yaml(
    system: &std::path::Path,
    services: &std::path::Path,
    bin: &std::path::Path,
    storage: &std::path::Path,
    web: &std::path::Path,
) -> String {
    format!(
        r#"name: test
display_name: Bootstrap Fixture Profile
target: local
site:
  name: testsite
  github_link: null
database:
  type: postgres
  external_db_access: false
server:
  host: 127.0.0.1
  port: 8080
  api_server_url: http://127.0.0.1
  api_internal_url: http://127.0.0.1
  api_external_url: http://127.0.0.1
  use_https: false
  cors_allowed_origins:
    - http://127.0.0.1
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
  web_path: {web}
  storage: {storage}
  geoip_database: null
security:
  jwt_issuer: test
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
governance:
  authz:
    hook:
      mode: unrestricted
      timeout_ms: 500
      acknowledgement: "{ack}"
"#,
        system = system.display(),
        services = services.display(),
        bin = bin.display(),
        storage = storage.display(),
        web = web.display(),
        ack = UNRESTRICTED_ACKNOWLEDGEMENT,
    )
}
