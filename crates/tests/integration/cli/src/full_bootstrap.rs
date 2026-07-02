//! Shared full-bootstrap fixture for subprocess tests.
//!
//! Builds a tempdir profile that passes the entire boot pipeline — profile,
//! secrets, paths, startup validation (web branding, AI providers, MCP
//! manifests), JWT signing key, and the system-admin row in the target
//! database — so subprocess tests reach real handler bodies instead of
//! dying inside `run_validation()` or `AppContext::new()`.
//!
//! The fixture is process-global (`OnceLock`); `admin bootstrap` is run once
//! against `DATABASE_URL` to satisfy the system-admin lookup. All helpers
//! return `None` when `DATABASE_URL` is unset so the suite degrades to a
//! no-op on machines without a test database.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use assert_cmd::Command;
use tempfile::TempDir;

pub const TEST_OAUTH_AT_REST_PEPPER: &str = "test_oauth_at_rest_pepper_for_bootstrap_fixture_zzz";
pub const TEST_MANIFEST_SIGNING_SEED: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
const UNRESTRICTED_ACKNOWLEDGEMENT: &str = "I understand this disables all authorization";

pub const FIXTURE_AGENT: &str = "covagent";
pub const FIXTURE_MCP_SERVER: &str = "fixture_mcp";

pub struct FullBootstrap {
    _tmp: TempDir,
    pub profile_path: PathBuf,
    pub services_dir: PathBuf,
    pub system_dir: PathBuf,
}

static FULL: OnceLock<Option<FullBootstrap>> = OnceLock::new();

pub fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok().filter(|v| !v.is_empty())
}

pub fn fixture() -> Option<&'static FullBootstrap> {
    FULL.get_or_init(|| database_url().map(|_| build()))
        .as_ref()
}

pub fn systemprompt_bin() -> PathBuf {
    if let Ok(path) = std::env::var("SYSTEMPROMPT_BIN") {
        let p = PathBuf::from(path);
        if p.exists() {
            return p;
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest_dir.ancestors() {
        for sub in [
            "target/debug/systemprompt",
            "crates/tests/target/debug/systemprompt",
        ] {
            let candidate = ancestor.join(sub);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    build_cli_binary()
}

fn build_cli_binary() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let repo_root = manifest_dir
                .ancestors()
                .find(|p| p.join("crates/entry/cli/Cargo.toml").exists())
                .expect("repo root with crates/entry/cli")
                .to_path_buf();
            let status = std::process::Command::new("cargo")
                .args(["build", "-p", "systemprompt-cli", "--bin", "systemprompt"])
                .current_dir(&repo_root)
                .status()
                .expect("spawn cargo build for systemprompt binary");
            assert!(status.success(), "cargo build -p systemprompt-cli failed");
            repo_root.join("target/debug/systemprompt")
        })
        .clone()
}

pub fn command_bare() -> Option<Command> {
    fixture()?;
    let mut c = Command::new(systemprompt_bin());
    c.env_remove("RUST_LOG");
    c.env_remove("SYSTEMPROMPT_PROFILE");
    c.env("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER);
    c.env("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED);
    c.env("SYSTEMPROMPT_SUBPROCESS", "1");
    c.arg("--non-interactive");
    c.arg("--no-color");
    c.timeout(std::time::Duration::from_secs(120));
    Some(c)
}

pub fn command() -> Option<Command> {
    let fixture = fixture()?;
    let mut c = Command::new(systemprompt_bin());
    c.env_remove("RUST_LOG");
    c.env_remove("SYSTEMPROMPT_PROFILE");
    c.env("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER);
    c.env("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED);
    c.env("SYSTEMPROMPT_SUBPROCESS", "1");
    c.arg("--non-interactive");
    c.arg("--no-color");
    c.arg("--profile").arg(&fixture.profile_path);
    c.timeout(std::time::Duration::from_secs(120));
    Some(c)
}

pub fn run(args: &[&str]) {
    let Some(mut cmd) = command() else { return };
    cmd.args(args);
    let _ = cmd.assert();
}

pub fn run_with_formats(args: &[&str]) {
    run(args);
    for format in ["--json", "--yaml"] {
        let Some(mut cmd) = command() else { return };
        cmd.arg(format);
        cmd.args(args);
        let _ = cmd.assert();
    }
}

fn build() -> FullBootstrap {
    let tmp = tempfile::tempdir().expect("create profile tempdir");
    let root = tmp.path().to_path_buf();

    let system_dir = root.join("system");
    let services_dir = root.join("services");

    for dir in [
        "system",
        "system/web",
        "services/config",
        "services/ai",
        "services/content",
        "services/web",
        "services/skills",
        "services/agents",
        "services/plugins",
        "services/hooks",
        "services/marketplaces",
        "bin",
        "storage",
    ] {
        std::fs::create_dir_all(root.join(dir)).expect("mkdir profile path");
    }

    std::fs::write(
        services_dir.join("config/config.yaml"),
        render_services_config(59999),
    )
    .expect("write services config");
    std::fs::write(services_dir.join("ai/config.yaml"), AI_CONFIG).expect("write ai config");
    std::fs::write(services_dir.join("content/config.yaml"), "{}\n").expect("write content stub");
    std::fs::write(services_dir.join("web/config.yaml"), WEB_CONFIG).expect("write web config");
    std::fs::write(services_dir.join("web/metadata.yaml"), "{}\n").expect("write metadata stub");

    write_signing_key(&system_dir.join("signing_key.pem"));

    std::fs::create_dir_all(root.join("covfix")).expect("mkdir profile dir");
    let profile_path = root.join("covfix/profile.yaml");
    std::fs::write(&profile_path, render_profile(&root)).expect("write profile.yaml");

    let fixture = FullBootstrap {
        _tmp: tmp,
        profile_path,
        services_dir,
        system_dir,
    };
    bootstrap_system_admin(&fixture);
    fixture
}

fn write_signing_key(path: &Path) {
    let key = systemprompt_security::keys::RsaSigningKey::generate_bits(2048)
        .expect("generate fixture signing key");
    key.write_pem_file(path).expect("write signing key pem");
}

fn bootstrap_system_admin(fixture: &FullBootstrap) {
    let mut c = Command::new(systemprompt_bin());
    c.env_remove("RUST_LOG");
    c.env_remove("SYSTEMPROMPT_PROFILE");
    c.env("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER);
    c.env("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED);
    c.env("SYSTEMPROMPT_SUBPROCESS", "1");
    c.arg("--non-interactive");
    c.arg("--no-color");
    c.arg("--profile").arg(&fixture.profile_path);
    c.args(["admin", "bootstrap", "--email", "testadmin@example.com"]);
    c.timeout(std::time::Duration::from_secs(120));
    // A concurrent test process may have created the admin already; either
    // exit status leaves the row in place, which is all the boot path needs.
    let _ = c.assert();
    normalize_admin_email();
}

/// A pre-existing `testadmin` row keeps its original email through bootstrap;
/// session-token generation rejects dot-less domains, so repair it in place.
fn normalize_admin_email() {
    let Some(url) = database_url() else { return };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build email-normalize runtime");
    runtime.block_on(async {
        let pool = sqlx::PgPool::connect(&url)
            .await
            .expect("connect to test database");
        sqlx::query(
            "UPDATE users SET email = 'testadmin@example.com'
             WHERE name = 'testadmin' AND email NOT LIKE '%.%'",
        )
        .execute(&pool)
        .await
        .expect("normalize admin email");
    });
}

pub fn rewrite_services_config(fixture: &FullBootstrap, mcp_port: u16) {
    std::fs::write(
        fixture.services_dir.join("config/config.yaml"),
        render_services_config(mcp_port),
    )
    .expect("rewrite services config");
}

fn render_services_config(mcp_port: u16) -> String {
    SERVICES_CONFIG_TEMPLATE.replace("@MCP_PORT@", &mcp_port.to_string())
}

const SERVICES_CONFIG_TEMPLATE: &str = r#"agents:
  covagent:
    name: covagent
    port: 4777
    endpoint: /api/v1/agents/covagent/
    enabled: false
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: covagent
      displayName: Coverage Agent
      description: Fixture agent for subprocess coverage tests
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: true
      defaultInputModes:
      - text/plain
      defaultOutputModes:
      - text/plain
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: You are a fixture agent.
      mcpServers:
        source: instance
      skills:
        source: instance
      provider: anthropic
      model: claude-sonnet-4-5
      toolModelOverrides: {}
    oauth:
      required: false
      scopes: []
      audience: a2a
mcp_servers:
  fixture_mcp:
    type: external
    binary: ""
    remote_endpoint: http://127.0.0.1:@MCP_PORT@/mcp
    package: fixture
    port: @MCP_PORT@
    enabled: true
    display_in_web: true
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
  providers:
    anthropic:
      enabled: true
      default_model: claude-sonnet-4-5
"#;

const AI_CONFIG: &str = r#"ai:
  default_provider: anthropic
  providers:
    anthropic:
      enabled: true
      default_model: claude-sonnet-4-5
    openai:
      enabled: false
      default_model: gpt-5
"#;

const WEB_CONFIG: &str = r#"branding:
  name: fixture-brand
  title: "Fixture Brand"
  description: Fixture brand for subprocess coverage tests
  copyright: 2026 fixture
  themeColor: '#ff0000'
  display_sitename: true
  twitter_handle: '@fixture'
  logo:
    primary:
      svg: /images/logo.svg
  favicon: /images/favicon.ico
fonts:
  body:
    family: OpenSans
    fallback: sans-serif
"#;

fn render_profile(root: &Path) -> String {
    format!(
        r#"name: subprocess_full
display_name: Subprocess Full Bootstrap Profile
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
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: https://api.anthropic.com/v1/messages
    api_key_secret: anthropic
    models:
      - id: claude-sonnet-4-5
  - name: openai
    wire: openai-chat
    surface: openai
    endpoint: https://api.openai.com/v1/chat/completions
    api_key_secret: openai
    models:
      - id: gpt-5
governance:
  authz:
    hook:
      mode: unrestricted
      timeout_ms: 500
      acknowledgement: "{ack}"
"#,
        system = root.join("system").display(),
        services = root.join("services").display(),
        bin = root.join("bin").display(),
        web = root.join("system/web").display(),
        storage = root.join("storage").display(),
        ack = UNRESTRICTED_ACKNOWLEDGEMENT,
    )
}
