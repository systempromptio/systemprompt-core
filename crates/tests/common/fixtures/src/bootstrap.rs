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

/// The Slack workspace id seeded into the fixture `config.yaml`. Tests sign
/// requests carrying this `team_id` so [`crate::messaging`] resolves the app.
pub const TEST_SLACK_WORKSPACE_ID: &str = "T_TEST_WS";
/// The Teams Entra tenant id seeded into the fixture `config.yaml`.
pub const TEST_TEAMS_TENANT_ID: &str = "tenant-test";
/// The Microsoft App (bot) id — the audience inbound Teams activity tokens
/// must carry, and the `client_id` for outbound token acquisition.
pub const TEST_TEAMS_APP_ID: &str = "app-test-1";
/// The agent both messaging apps route to. A `services` backend row plus the
/// matching `config.yaml` entry (`oauth.required = false`) make it
/// dispatchable.
pub const TEST_MESSAGING_AGENT: &str = "test_messaging_agent";

/// The Slack signing secret resolved from the named ref `slack_signing_secret`.
pub const TEST_SLACK_SIGNING_SECRET: &str = "test-slack-signing-secret-value";
/// The Slack bot token resolved from the named ref `slack_bot_token`.
pub const TEST_SLACK_BOT_TOKEN: &str = "xoxb-test-bot-token";
/// The Teams app password resolved from the named ref `teams_app_password`.
pub const TEST_TEAMS_APP_PASSWORD: &str = "test-teams-app-password";

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

    std::fs::write(
        services_path.join("config/config.yaml"),
        messaging_config_yaml(),
    )
    .expect("write services config.yaml");
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
        // Named secrets the messaging apps reference. The fixture runs in
        // subprocess mode, so the secrets singleton loads from the environment;
        // `SYSTEMPROMPT_CUSTOM_SECRETS` lists the extra keys to pull through.
        if env::var("SYSTEMPROMPT_CUSTOM_SECRETS").is_err() {
            env::set_var(
                "SYSTEMPROMPT_CUSTOM_SECRETS",
                "slack_signing_secret,slack_bot_token,teams_app_password",
            );
            env::set_var("slack_signing_secret", TEST_SLACK_SIGNING_SECRET);
            env::set_var("slack_bot_token", TEST_SLACK_BOT_TOKEN);
            env::set_var("teams_app_password", TEST_TEAMS_APP_PASSWORD);
        }
    }
}

fn write_yaml_stub(path: &std::path::Path) {
    std::fs::write(path, "{}\n").expect("write yaml stub");
}

/// The seeded services config: one dispatchable agent (`oauth.required =
/// false`) plus one Slack app and one Teams app routing to it.
/// `ConfigLoader::load()` reads exactly this file, so the messaging routes
/// resolve their app, agent, and named secrets from a single deterministic
/// source.
fn messaging_config_yaml() -> String {
    format!(
        r#"agents:
  {agent}:
    name: {agent}
    port: 9250
    endpoint: http://127.0.0.1:9250
    enabled: true
    card:
      protocolVersion: "0.3.0"
      displayName: Test Messaging Agent
      description: Agent backend for messaging dispatch tests.
      version: "1.0.0"
    metadata: {{}}
    oauth:
      required: false
slack_apps:
  test_slack:
    workspace_id: {slack_ws}
    signing_secret_ref: slack_signing_secret
    bot_token_ref: slack_bot_token
    enabled: true
    default_agent: {agent}
    authz:
      allowed_roles:
        - user
teams_apps:
  test_teams:
    tenant_id: {teams_tenant}
    app_id: {teams_app}
    app_password_ref: teams_app_password
    enabled: true
    default_agent: {agent}
    authz:
      allowed_roles:
        - user
"#,
        agent = TEST_MESSAGING_AGENT,
        slack_ws = TEST_SLACK_WORKSPACE_ID,
        teams_tenant = TEST_TEAMS_TENANT_ID,
        teams_app = TEST_TEAMS_APP_ID,
    )
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
