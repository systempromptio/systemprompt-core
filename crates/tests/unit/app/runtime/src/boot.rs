//! Full in-process bootstrap fixture: a tempdir profile tree (system,
//! services, storage, bin), a secrets.json wired to the test database, and
//! ProfileBootstrap + SecretsBootstrap initialisation. Each test that uses
//! this runs in its own nextest process, so the global OnceLock singletons
//! are safe to install per-test.

use std::path::Path;

use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_models::profile::UNRESTRICTED_ACKNOWLEDGEMENT;
use tempfile::TempDir;

pub const PEPPER: &str = "test_oauth_at_rest_pepper_for_runtime_boot_fixture";
pub const SEED: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

pub struct BootFixture {
    pub _tmp: TempDir,
    pub database_url: String,
}

pub fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("TEST_DATABASE_URL"))
        .ok()
        .filter(|v| !v.is_empty())
}

pub struct BootOptions {
    pub admin_username: String,
    pub stream_per_second: u32,
    pub pool_settings: bool,
    pub write_url: bool,
    pub mcp_servers_yaml: String,
    pub broken_storage: bool,
}

impl Default for BootOptions {
    fn default() -> Self {
        Self {
            admin_username: "testadmin".to_owned(),
            stream_per_second: 100,
            pool_settings: false,
            write_url: false,
            mcp_servers_yaml: "mcp_servers: {}\n".to_owned(),
            broken_storage: false,
        }
    }
}

// Builds the tree and installs ProfileBootstrap + SecretsBootstrap.
// Returns None when no test database is configured, so suites degrade to
// a no-op on machines without Postgres.
pub fn boot(opts: &BootOptions) -> Option<BootFixture> {
    let database_url = database_url()?;
    let tmp = tempfile::tempdir().expect("create boot tempdir");
    let root = tmp.path();

    for dir in [
        "system",
        "system/web",
        "services/config",
        "services/ai",
        "services/content",
        "services/web",
        "services/skills/echo_skill",
        "services/agents",
        "bin",
        "storage/files/covassets_ok",
    ] {
        std::fs::create_dir_all(root.join(dir)).expect("mkdir fixture path");
    }

    let system_dir = root.join("system");
    let services_dir = root.join("services");
    let storage_dir = root.join("storage");

    std::fs::write(
        services_dir.join("config/config.yaml"),
        services_config(&opts.mcp_servers_yaml),
    )
    .expect("write services config");
    std::fs::write(services_dir.join("ai/config.yaml"), AI_CONFIG).expect("write ai config");
    std::fs::write(
        services_dir.join("skills/echo_skill/config.yaml"),
        "id: echo_skill\nname: Echo Skill\ndescription: Boot fixture skill\n",
    )
    .expect("write skill config");
    std::fs::write(
        services_dir.join("skills/echo_skill/index.md"),
        "# Echo Skill\n\nEcho the input back.\n",
    )
    .expect("write skill content");
    std::fs::write(services_dir.join("content/config.yaml"), "{}\n").expect("write content stub");
    std::fs::write(services_dir.join("web/config.yaml"), WEB_CONFIG).expect("write web config");
    std::fs::write(services_dir.join("web/metadata.yaml"), "{}\n").expect("write metadata stub");
    std::fs::write(
        storage_dir.join("files/covassets_ok/present.css"),
        "body{}\n",
    )
    .expect("write present asset");

    write_signing_key(&system_dir.join("signing_key.pem"));

    let secrets_path = root.join("secrets.json");
    let write_url_json = if opts.write_url {
        format!(",\n  \"database_write_url\": \"{database_url}\"")
    } else {
        String::new()
    };
    std::fs::write(
        &secrets_path,
        format!(
            "{{\n  \"oauth_at_rest_pepper\": \"{PEPPER}\",\n  \
             \"manifest_signing_secret_seed\": \"{SEED}\",\n  \
             \"database_url\": \"{database_url}\"{write_url_json}\n}}\n"
        ),
    )
    .expect("write secrets.json");

    let storage_profile_dir = if opts.broken_storage {
        let blocker = root.join("blocker");
        std::fs::write(&blocker, b"not a directory").expect("write storage blocker");
        blocker.join("storage")
    } else {
        storage_dir.clone()
    };

    let profile_path = root.join("profile.yaml");
    std::fs::write(
        &profile_path,
        profile_yaml(root, opts, &storage_profile_dir),
    )
    .expect("write profile.yaml");

    ProfileBootstrap::init_from_path(&profile_path).expect("init profile bootstrap");
    SecretsBootstrap::init().expect("init secrets bootstrap");

    Some(BootFixture {
        _tmp: tmp,
        database_url,
    })
}

fn write_signing_key(path: &Path) {
    let key = systemprompt_security::keys::RsaSigningKey::generate_bits(2048)
        .expect("generate fixture signing key");
    key.write_pem_file(path).expect("write signing key pem");
}

fn services_config(mcp_servers_yaml: &str) -> String {
    format!(
        r"agents:
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
      description: Boot fixture agent
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
      toolModelOverrides: {{}}
    oauth:
      required: false
      scopes: []
      audience: a2a
{mcp_servers_yaml}settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
  providers:
    anthropic:
      enabled: true
      default_model: claude-sonnet-4-5
"
    )
}

const AI_CONFIG: &str = r"ai:
  default_provider: anthropic
  providers:
    anthropic:
      enabled: true
      default_model: claude-sonnet-4-5
";

const WEB_CONFIG: &str = r"branding:
  name: fixture-brand
  title: Fixture Brand
  description: Boot fixture brand
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
";

fn profile_yaml(root: &Path, opts: &BootOptions, storage: &Path) -> String {
    let pool = if opts.pool_settings {
        "  pool:\n    max_connections: 4\n    acquire_timeout_secs: 20\n    idle_timeout_secs: \
         300\n    max_lifetime_secs: 1200\n"
    } else {
        ""
    };
    format!(
        r"name: runtime_boot
display_name: Runtime Boot Fixture
target: local
site:
  name: testsite
  github_link: null
database:
  type: postgres
  external_db_access: false
{pool}server:
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
  stream_per_second: {stream_per_second}
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
secrets:
  secrets_path: secrets.json
  source: file
governance:
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
        storage = storage.display(),
        stream_per_second = opts.stream_per_second,
        admin = opts.admin_username,
        ack = UNRESTRICTED_ACKNOWLEDGEMENT,
    )
}
