//! A tempdir-backed profile tree whose `services.sources` a test controls.
//!
//! `services refresh` and `admin config secret check` both read the profile
//! singleton, so the tree is written to disk and installed through
//! `ProfileBootstrap::init_from_path`. nextest runs one process per test, so
//! installing the singleton per test is safe.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use tempfile::TempDir;


pub const SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub struct ProfileTree {
    pub _tmp: TempDir,
    pub root: PathBuf,
    pub profile_path: PathBuf,
}

pub fn https_sources_block(entries: &[(&str, &str)]) -> String {
    if entries.is_empty() {
        return "services:\n  sources: []\n".to_owned();
    }
    let mut block = String::from("services:\n  sources:\n");
    for (name, url) in entries {
        block.push_str(&format!(
            "    - name: {name}\n      https:\n        url: {url}\n        verify:\n          \
             sha256: {SHA256}\n"
        ));
    }
    block
}

pub fn write_tree(services_block: &str, secrets_section: &str) -> ProfileTree {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    for dir in [
        "system",
        "system/web",
        "services/config",
        "services/content",
        "services/web",
        "bin",
        "storage",
        "cache",
    ] {
        std::fs::create_dir_all(root.join(dir)).expect("mkdir");
    }
    std::fs::write(root.join("services/config/config.yaml"), "settings: {}\n").expect("write");
    std::fs::write(root.join("services/content/config.yaml"), "{}\n").expect("write");
    std::fs::write(root.join("services/web/config.yaml"), "branding: {}\n").expect("write");
    std::fs::write(root.join("services/web/metadata.yaml"), "{}\n").expect("write");

    let cache_dir = root.join("cache");
    let profile_path = root.join("profile.yaml");
    std::fs::write(
        &profile_path,
        profile_yaml(&root, &cache_dir, services_block, secrets_section),
    )
    .expect("write profile.yaml");

    ProfileTree {
        _tmp: tmp,
        root,
        profile_path,
    }
}

fn profile_yaml(
    root: &Path,
    cache_dir: &Path,
    services_block: &str,
    secrets_section: &str,
) -> String {
    let mut services = services_block.to_owned();
    services.push_str(&format!("  cache_dir: {}\n", cache_dir.display()));
    format!(
        r"name: services_fixture
display_name: Services Fixture
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
  services: {svc}
  bin: {bin}
  web_path: {web}
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
system_admin:
  username: testadmin
runtime:
  environment: development
  log_level: quiet
  output_format: text
  no_color: true
  non_interactive: true
{services}{secrets_section}governance:
  authz:
    hook:
      mode: unrestricted
      timeout_ms: 500
      acknowledgement: '{ack}'
",
        system = root.join("system").display(),
        svc = root.join("services").display(),
        bin = root.join("bin").display(),
        web = root.join("system/web").display(),
        storage = root.join("storage").display(),
        ack = systemprompt_models::profile::UNRESTRICTED_ACKNOWLEDGEMENT,
    )
}

pub fn set_env(key: &str, value: &str) {
    unsafe { std::env::set_var(key, value) };
}

pub fn loaded(services_block: &str) -> (ProfileTree, systemprompt_models::Profile) {
    let tree = write_tree(
        services_block,
        "secrets:\n  secrets_path: secrets.json\n  source: env\n",
    );
    let body = std::fs::read_to_string(&tree.profile_path).expect("read profile.yaml");
    let profile =
        systemprompt_models::Profile::from_yaml(&body, &tree.profile_path).expect("profile parses");
    (tree, profile)
}

pub fn https_source_with_keys(name: &str, url: &str, keys: &[String]) -> String {
    let mut block = format!(
        "services:\n  sources:\n    - name: {name}\n      https:\n        url: {url}\n        verify:\n          ed25519_public_keys:\n"
    );
    if keys.is_empty() {
        block = format!(
            "services:\n  sources:\n    - name: {name}\n      https:\n        url: {url}\n        verify:\n          ed25519_public_keys: []\n"
        );
        return block;
    }
    for key in keys {
        block.push_str(&format!("            - {key}\n"));
    }
    block
}
