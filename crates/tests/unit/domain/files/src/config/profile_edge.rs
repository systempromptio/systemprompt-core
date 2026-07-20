//! `ensure_storage_structure` root-creation failure, driven by a custom
//! profile whose storage root sits below a regular file. Initialises
//! `ProfileBootstrap` itself, so it must not share a process with the
//! bootstrap fixture (nextest runs one process per test).

use systemprompt_config::ProfileBootstrap;
use systemprompt_files::FilesConfig;
use systemprompt_models::AppPaths;
use systemprompt_models::profile::UNRESTRICTED_ACKNOWLEDGEMENT;

fn profile_yaml(system: &std::path::Path, storage: &std::path::Path) -> String {
    let services = system.join("services");
    let bin = system.join("bin");
    let web = system.join("web");
    format!(
        r"name: test
display_name: Files Edge Profile
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
governance:
  authz:
    hook:
      mode: unrestricted
      timeout_ms: 500
      acknowledgement: '{ack}'
",
        system = system.display(),
        services = services.display(),
        bin = bin.display(),
        web = web.display(),
        storage = storage.display(),
        ack = UNRESTRICTED_ACKNOWLEDGEMENT,
    )
}

#[test]
fn ensure_storage_structure_reports_uncreatable_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let system = tmp.path().join("system");
    std::fs::create_dir_all(system.join("services/config")).expect("mkdir services");
    std::fs::create_dir_all(system.join("bin")).expect("mkdir bin");
    std::fs::create_dir_all(system.join("web")).expect("mkdir web");

    let blocker = tmp.path().join("blocker");
    std::fs::write(&blocker, b"not a directory").expect("write blocker");
    let storage = blocker.join("storage");

    let profile_path = tmp.path().join("profile.yaml");
    std::fs::write(&profile_path, profile_yaml(&system, &storage)).expect("write profile");

    let profile = ProfileBootstrap::init_from_path(&profile_path).expect("init profile");
    let paths = AppPaths::from_profile(&profile.paths).expect("app paths");
    let cfg = FilesConfig::from_profile(&paths).expect("from_profile");

    let errors = cfg.ensure_storage_structure();

    assert_eq!(errors.len(), 1, "unexpected errors: {errors:?}");
    assert!(errors[0].contains("Failed to create storage root"));
    assert!(errors[0].contains(&storage.display().to_string()));
}
