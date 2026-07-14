//! Shared tempdir profile-tree fixture for bootstrap and config-loader
//! tests. Each nextest test runs in its own process, so installing the
//! ProfileBootstrap / SecretsBootstrap / Config singletons per-test is safe.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub const PEPPER: &str = "test_oauth_at_rest_pepper_for_config_fixture";
pub const SEED: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
pub const DB_URL: &str = "postgresql://user:pass@localhost:5432/config_fixture";

pub struct Fixture {
    pub tmp: TempDir,
    pub profile_path: PathBuf,
    pub secrets_path: PathBuf,
}

pub fn secrets_json(seed: Option<&str>) -> String {
    let seed_line = seed.map_or(String::new(), |s| {
        format!(",\n  \"manifest_signing_secret_seed\": \"{s}\"")
    });
    format!(
        "{{\n  \"oauth_at_rest_pepper\": \"{PEPPER}\",\n  \"database_url\": \
         \"{DB_URL}\"{seed_line}\n}}\n"
    )
}

pub fn write_tree(secrets_section: &str, secrets_body: Option<&str>) -> Fixture {
    let tmp = tempfile::tempdir().expect("create fixture tempdir");
    let root = tmp.path();

    for dir in [
        "system",
        "system/web",
        "services/config",
        "services/content",
        "services/web",
        "services/skills/echo_skill",
        "bin",
        "storage",
    ] {
        std::fs::create_dir_all(root.join(dir)).expect("mkdir fixture path");
    }
    std::fs::write(root.join("services/config/config.yaml"), "settings: {}\n")
        .expect("write services config");
    std::fs::write(root.join("services/content/config.yaml"), "{}\n").expect("write content stub");
    std::fs::write(root.join("services/web/config.yaml"), "branding: {}\n")
        .expect("write web config");
    std::fs::write(root.join("services/web/metadata.yaml"), "{}\n").expect("write metadata stub");
    std::fs::write(
        root.join("services/skills/echo_skill/config.yaml"),
        "id: echo_skill\nname: Echo\ndescription: fixture skill\n",
    )
    .expect("write skill config");
    std::fs::write(root.join("services/skills/echo_skill/index.md"), "# Echo\n")
        .expect("write skill content");

    let secrets_path = root.join("secrets.json");
    if let Some(body) = secrets_body {
        std::fs::write(&secrets_path, body).expect("write secrets.json");
    }

    let profile_path = root.join("profile.yaml");
    std::fs::write(
        &profile_path,
        profile_yaml(root, secrets_section, "testadmin"),
    )
    .expect("write profile.yaml");

    Fixture {
        tmp,
        profile_path,
        secrets_path,
    }
}

pub fn profile_yaml(root: &Path, secrets_section: &str, admin: &str) -> String {
    format!(
        r"name: config_fixture
display_name: Config Fixture
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
  disabled: false
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
  username: {admin}
runtime:
  environment: development
  log_level: quiet
  output_format: text
  no_color: true
  non_interactive: true
{secrets_section}governance:
  authz:
    hook:
      mode: unrestricted
      timeout_ms: 500
      acknowledgement: '{ack}'
",
        system = root.join("system").display(),
        services = root.join("services").display(),
        bin = root.join("bin").display(),
        web = root.join("system/web").display(),
        storage = root.join("storage").display(),
        ack = systemprompt_models::profile::UNRESTRICTED_ACKNOWLEDGEMENT,
    )
}

pub const FILE_SECRETS: &str = "secrets:\n  secrets_path: secrets.json\n  source: file\n";
pub const ENV_SECRETS: &str = "secrets:\n  secrets_path: secrets.json\n  source: env\n";

pub fn set_env(key: &str, value: &str) {
    // nextest runs one process per test, so mutating the environment cannot
    // leak into other tests.
    unsafe { std::env::set_var(key, value) };
}

pub fn remove_env(key: &str) {
    unsafe { std::env::remove_var(key) };
}
