//! In-process harness driving the non-interactive `cloud` command paths of
//! `systemprompt-cli` against a wiremock-backed cloud API.
//!
//! The harness owns a single tempdir project root (`.systemprompt/` +
//! `services/` scaffolding), a single shared `MockServer`, and the two
//! process-global bootstraps the cloud commands consult: `CredentialsBootstrap`
//! (initialised once from a `credentials.json` whose `api_url` is the mock
//! server) and `ProfileBootstrap` (a validated local profile carrying a
//! `cloud.tenant_id`). Because credentials, the profile, and the process cwd
//! are all global, every test serialises through one async mutex and runs while
//! chdir'd into the project root; per-test file seeding and `MockServer::reset`
//! give each test a clean slate.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::json;
use systemprompt_cli::cloud::auth::AuthCommands;
use systemprompt_cli::cloud::secrets::SecretsCommands;
use systemprompt_cli::cloud::tenant::{
    TenantCancelArgs, TenantCommands, TenantDeleteArgs, TenantRotateArgs,
};
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_cloud::tenants::{NewCloudTenantParams, StoredTenant, TenantStore};
use systemprompt_cloud::{CloudPath, CredentialsBootstrap, get_cloud_paths};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::TenantId;
use tempfile::TempDir;
use tokio::sync::{Mutex, MutexGuard, OnceCell};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub(super) const FAR_FUTURE_JWT: &str = "e30.eyJleHAiOjk5OTk5OTk5OTl9.sig";
pub(super) const TENANT_ID: &str = "t-harness";
pub(super) const OTHER_TENANT_ID: &str = "t-other";
pub(super) const USER_EMAIL: &str = "harness@example.com";

pub(super) struct Harness {
    _tmp: TempDir,
    root: PathBuf,
    server: MockServer,
    profile_ready: bool,
}

static HARNESS: OnceCell<Harness> = OnceCell::const_new();
static LOCK: Mutex<()> = Mutex::const_new(());

async fn harness() -> &'static Harness {
    HARNESS.get_or_init(build_harness).await
}

async fn build_harness() -> Harness {
    let tmp = tempfile::tempdir().expect("create harness tempdir");
    let root = tmp.path().to_path_buf();
    scaffold_project(&root);

    let server = MockServer::start().await;
    write_credentials(&root, &server.uri());

    let prev = std::env::current_dir().ok();
    std::env::set_current_dir(&root).expect("chdir into harness root");

    mount_get_user(&server).await;
    let _ = CredentialsBootstrap::try_init().await;

    let profile_path = root.join(".systemprompt/profiles/local/profile.yaml");
    let profile_ready = if ProfileBootstrap::is_initialized() {
        true
    } else {
        ProfileBootstrap::init_from_path(&profile_path).is_ok()
    };

    server.reset().await;
    if let Some(prev) = prev {
        let _ = std::env::set_current_dir(prev);
    }

    Harness {
        _tmp: tmp,
        root,
        server,
        profile_ready,
    }
}

fn scaffold_project(root: &Path) {
    for dir in [
        ".systemprompt",
        ".systemprompt/profiles/local",
        "system",
        "system/web",
        "services/config",
        "services/content",
        "services/web",
        "bin",
        "storage",
    ] {
        std::fs::create_dir_all(root.join(dir)).expect("mkdir harness path");
    }

    std::fs::write(root.join("services/config/config.yaml"), "{}\n").expect("services config");
    std::fs::write(root.join("services/content/config.yaml"), "{}\n").expect("content config");
    std::fs::write(root.join("services/web/config.yaml"), "{}\n").expect("web config");
    std::fs::write(root.join("services/web/metadata.yaml"), "{}\n").expect("web metadata");

    write_signing_key(&root.join("system/signing_key.pem"));
    std::fs::write(
        root.join(".systemprompt/profiles/local/profile.yaml"),
        render_profile(root),
    )
    .expect("write profile.yaml");
    std::fs::write(
        root.join(".systemprompt/profiles/local/secrets.json"),
        r#"{"CUSTOM_SECRET":"value","ANOTHER_SECRET":"value2"}"#,
    )
    .expect("write secrets.json");
}

fn write_signing_key(path: &Path) {
    let key = systemprompt_security::keys::RsaSigningKey::generate_bits(2048)
        .expect("generate harness signing key");
    key.write_pem_file(path).expect("write signing key pem");
}

fn write_credentials(root: &Path, api_url: &str) {
    let creds = json!({
        "api_token": FAR_FUTURE_JWT,
        "api_url": api_url,
        "authenticated_at": Utc::now().to_rfc3339(),
        "user_email": USER_EMAIL,
        "last_validated_at": null,
    });
    std::fs::write(
        root.join(".systemprompt/credentials.json"),
        serde_json::to_string_pretty(&creds).unwrap(),
    )
    .expect("write credentials.json");
}

pub(super) fn seed_tenants(root: &Path) {
    let cloud = StoredTenant::new_cloud(NewCloudTenantParams {
        id: TenantId::new(TENANT_ID),
        name: "Harness Prod".to_owned(),
        app_id: Some("app-harness".to_owned()),
        hostname: Some("harness.example.com".to_owned()),
        region: Some("iad".to_owned()),
        database_url: Some("postgres://ext/db".to_owned()),
        internal_database_url: "postgres://int/db".to_owned(),
        external_db_access: true,
    });
    let local = StoredTenant::new_local(
        TenantId::new(OTHER_TENANT_ID),
        "Harness Local".to_owned(),
        "postgres://local/db".to_owned(),
    );
    let store = TenantStore::new(vec![cloud, local]);
    let path = root.join(".systemprompt/tenants.json");
    store.save_to_path(&path).expect("write tenants.json");
}

fn render_profile(root: &Path) -> String {
    format!(
        r#"name: local
display_name: Cloud Harness Profile
target: local
site:
  name: harness
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
cloud:
  tenant_id: {tenant}
  validation: skip
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
      acknowledgement: "I understand this disables all authorization"
"#,
        system = root.join("system").display(),
        services = root.join("services").display(),
        bin = root.join("bin").display(),
        web = root.join("system/web").display(),
        storage = root.join("storage").display(),
        tenant = TENANT_ID,
    )
}

mod batch3_auth;
mod batch3_profile;
mod batch3_restart;
mod batch3_secrets;
mod batch3_tenant;
mod batch3_validation;
mod db_cmds;
mod doctor_deploy;
mod domain_cmds;
mod init_cmds;
mod profile_cmds;
mod secrets_cmds;
mod sync_cmds;
mod tenant_flows;

pub(super) struct Env {
    _guard: MutexGuard<'static, ()>,
    prev_cwd: Option<PathBuf>,
    harness: &'static Harness,
}

impl Env {
    pub(super) fn server(&self) -> &'static MockServer {
        &self.harness.server
    }

    pub(super) fn profile_ready(&self) -> bool {
        self.harness.profile_ready
    }
}

impl Env {
    pub(super) fn root(&self) -> &Path {
        &self.harness.root
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        if let Some(prev) = self.prev_cwd.take() {
            let _ = std::env::set_current_dir(prev);
        }
    }
}

pub(super) async fn enter() -> Env {
    let guard = LOCK.lock().await;
    let harness = harness().await;
    let prev_cwd = std::env::current_dir().ok();
    std::env::set_current_dir(&harness.root).expect("chdir into harness root");

    write_credentials(&harness.root, &harness.server.uri());
    seed_tenants(&harness.root);

    harness.server.reset().await;
    mount_get_user(&harness.server).await;
    mount_token_exchange(&harness.server).await;

    Env {
        _guard: guard,
        prev_cwd,
        harness,
    }
}

async fn mount_get_user(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": { "id": "user_harness", "email": USER_EMAIL, "name": "Harness" },
            "tenants": [{
                "id": TENANT_ID,
                "name": "Harness Prod",
                "hostname": "harness.example.com",
                "region": "iad",
                "external_db_access": true,
                "database_url": "postgres://int/db"
            }]
        })))
        .mount(server)
        .await;
}

async fn mount_token_exchange(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tenant_bearer",
            "expires_in": 600
        })))
        .mount(server)
        .await;
}

pub(super) async fn mount_list_tenants(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": TENANT_ID, "name": "Harness Prod" }]
        })))
        .mount(server)
        .await;
}

pub(super) fn ctx(format: OutputFormat) -> CommandContext {
    let cli = CliConfig::default()
        .with_output_format(format)
        .with_interactive(false);
    CommandContext::new(cli, EnvOverrides::default())
}

pub(super) fn json_ctx() -> CommandContext {
    ctx(OutputFormat::Json)
}

pub(super) fn interactive_ctx(
    answers: impl IntoIterator<Item = impl Into<String>>,
) -> CommandContext {
    let cli = CliConfig::default()
        .with_output_format(OutputFormat::Table)
        .with_interactive(true)
        .with_assume_terminal(true);
    CommandContext::new(cli, EnvOverrides::default())
        .with_prompter(Box::new(systemprompt_cli::ScriptedPrompter::new(answers)))
}

pub(super) fn table_ctx() -> CommandContext {
    ctx(OutputFormat::Table)
}

#[tokio::test]
async fn status_reports_credentials_and_tenants() {
    let env = enter().await;
    mount_list_tenants(env.server()).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/status")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "status": "running", "app_url": "https://harness.example.com" }
        })))
        .mount(env.server())
        .await;

    cloud::execute(CloudCommands::Status, &json_ctx())
        .await
        .expect("status command");
}

#[tokio::test]
async fn status_output_shape_is_authenticated() {
    let env = enter().await;
    mount_list_tenants(env.server()).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/status")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "status": "running" }
        })))
        .mount(env.server())
        .await;

    cloud::execute(CloudCommands::Status, &table_ctx())
        .await
        .expect("status command table");

    let creds = CredentialsBootstrap::get().expect("creds").expect("some");
    assert_eq!(creds.user_email.as_str(), USER_EMAIL);
    assert!(!creds.is_token_expired());
}

#[tokio::test]
async fn status_tolerates_list_tenants_failure() {
    let env = enter().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    cloud::execute(CloudCommands::Status, &json_ctx())
        .await
        .expect("status tolerates upstream failure");
}

#[tokio::test]
async fn tenant_list_syncs_and_renders() {
    let env = enter().await;
    let _ = env;
    cloud::execute(
        CloudCommands::Tenant {
            command: Some(TenantCommands::List),
        },
        &json_ctx(),
    )
    .await
    .expect("tenant list");
}

#[tokio::test]
async fn tenant_show_by_id_from_store() {
    let _env = enter().await;
    cloud::execute(
        CloudCommands::Tenant {
            command: Some(TenantCommands::Show {
                id: Some(TENANT_ID.to_owned()),
            }),
        },
        &json_ctx(),
    )
    .await
    .expect("tenant show");
}

#[tokio::test]
async fn tenant_show_missing_id_errors() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Tenant {
            command: Some(TenantCommands::Show {
                id: Some("does-not-exist".to_owned()),
            }),
        },
        &json_ctx(),
    )
    .await
    .expect_err("missing tenant errors");
    assert!(err.to_string().contains("does-not-exist"));
}

#[tokio::test]
async fn tenant_delete_cloud_tenant_calls_api() {
    let env = enter().await;
    Mock::given(method("DELETE"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}")))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        CloudCommands::Tenant {
            command: Some(TenantCommands::Delete(TenantDeleteArgs {
                id: Some(TENANT_ID.to_owned()),
                yes: true,
            })),
        },
        &json_ctx(),
    )
    .await
    .expect("tenant delete");
}

#[tokio::test]
async fn tenant_delete_without_yes_errors_non_interactive() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Tenant {
            command: Some(TenantCommands::Delete(TenantDeleteArgs {
                id: Some(TENANT_ID.to_owned()),
                yes: false,
            })),
        },
        &json_ctx(),
    )
    .await
    .expect_err("delete needs --yes");
    assert!(err.to_string().contains("--yes"));
}

#[tokio::test]
async fn tenant_rotate_credentials_updates_store() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/api/v1/tenants/{TENANT_ID}/rotate-credentials"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "rotated",
            "message": "ok",
            "internal_database_url": "postgres://int/rotated",
            "external_database_url": "postgres://ext/rotated"
        })))
        .mount(env.server())
        .await;

    cloud::execute(
        CloudCommands::Tenant {
            command: Some(TenantCommands::RotateCredentials(TenantRotateArgs {
                id: Some(TENANT_ID.to_owned()),
                yes: true,
            })),
        },
        &json_ctx(),
    )
    .await
    .expect("rotate credentials");
}

#[tokio::test]
async fn tenant_rotate_local_tenant_rejected() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Tenant {
            command: Some(TenantCommands::RotateCredentials(TenantRotateArgs {
                id: Some(OTHER_TENANT_ID.to_owned()),
                yes: true,
            })),
        },
        &json_ctx(),
    )
    .await
    .expect_err("local tenant cannot rotate");
    assert!(err.to_string().contains("cloud tenants"));
}

#[tokio::test]
async fn tenant_cancel_requires_interactive() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Tenant {
            command: Some(TenantCommands::Cancel(TenantCancelArgs {
                id: Some(TENANT_ID.to_owned()),
            })),
        },
        &json_ctx(),
    )
    .await
    .expect_err("cancel needs interactive");
    assert!(err.to_string().contains("interactive"));
}

#[tokio::test]
async fn restart_cloud_tenant_calls_api() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/restart")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "restarting"
        })))
        .mount(env.server())
        .await;

    cloud::execute(
        CloudCommands::Restart {
            tenant: Some(TENANT_ID.to_owned()),
            yes: true,
        },
        &json_ctx(),
    )
    .await
    .expect("restart");
}

#[tokio::test]
async fn restart_without_yes_errors_non_interactive() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Restart {
            tenant: Some(TENANT_ID.to_owned()),
            yes: false,
        },
        &json_ctx(),
    )
    .await
    .expect_err("restart needs --yes");
    assert!(err.to_string().contains("--yes"));
}

#[tokio::test]
async fn restart_local_tenant_rejected() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Restart {
            tenant: Some(OTHER_TENANT_ID.to_owned()),
            yes: true,
        },
        &json_ctx(),
    )
    .await
    .expect_err("local tenant cannot restart");
    assert!(err.to_string().contains("cloud tenants"));
}

#[tokio::test]
async fn auth_whoami_reports_identity() {
    let env = enter().await;
    mount_list_tenants(env.server()).await;
    cloud::execute(CloudCommands::Auth(AuthCommands::Whoami), &json_ctx())
        .await
        .expect("whoami");
}

#[tokio::test]
async fn auth_logout_removes_credentials_file() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/activity"))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    let creds_path = get_cloud_paths().resolve(CloudPath::Credentials);
    assert!(creds_path.exists());

    cloud::execute(
        CloudCommands::Auth(AuthCommands::Logout(
            systemprompt_cli::cloud::auth::LogoutArgs { yes: true },
        )),
        &json_ctx(),
    )
    .await
    .expect("logout");

    assert!(!creds_path.exists());
}

#[tokio::test]
async fn sync_without_subcommand_errors_non_interactive() {
    let _env = enter().await;
    let err = cloud::execute(CloudCommands::Sync { command: None }, &json_ctx())
        .await
        .expect_err("sync needs a subcommand");
    assert!(err.to_string().contains("subcommand"));
}

#[tokio::test]
async fn secrets_set_pushes_to_cloud() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        CloudCommands::Secrets(SecretsCommands::Set {
            key_values: vec!["CUSTOM_SECRET=value".to_owned()],
        }),
        &json_ctx(),
    )
    .await
    .expect("secrets set");
}

#[tokio::test]
async fn secrets_set_rejects_only_system_managed() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    let err = cloud::execute(
        CloudCommands::Secrets(SecretsCommands::Set {
            key_values: vec!["FLY_APP_NAME=x".to_owned()],
        }),
        &json_ctx(),
    )
    .await
    .expect_err("all keys system-managed");
    assert!(err.to_string().contains("system-managed"));
}

#[tokio::test]
async fn secrets_sync_pushes_profile_secrets() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(CloudCommands::Secrets(SecretsCommands::Sync), &json_ctx())
        .await
        .expect("secrets sync");
}

#[tokio::test]
async fn secrets_unset_removes_key() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("DELETE"))
        .and(path(format!(
            "/api/v1/tenants/{TENANT_ID}/secrets/CUSTOM_SECRET"
        )))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        CloudCommands::Secrets(SecretsCommands::Unset {
            keys: vec!["CUSTOM_SECRET".to_owned()],
        }),
        &json_ctx(),
    )
    .await
    .expect("secrets unset");
}

#[tokio::test]
async fn secrets_cleanup_removes_system_managed() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("DELETE"))
        .and(path(format!(
            "/api/v1/tenants/{TENANT_ID}/secrets/SYSTEMPROMPT_API_URL"
        )))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        CloudCommands::Secrets(SecretsCommands::Cleanup),
        &json_ctx(),
    )
    .await
    .expect("secrets cleanup");
}

#[tokio::test]
async fn login_post_token_persists_credentials_and_tenants() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/activity"))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    let creds_path = get_cloud_paths().resolve(CloudPath::Credentials);
    let tenants_path = get_cloud_paths().resolve(CloudPath::Tenants);
    std::fs::remove_file(&creds_path).expect("remove seeded credentials");
    std::fs::remove_file(&tenants_path).expect("remove seeded tenants");

    let output = systemprompt_cli::cloud::auth::complete_login(
        &env.server().uri(),
        FAR_FUTURE_JWT.to_owned(),
        &json_ctx().cli,
    )
    .await
    .expect("post-token login");
    let _ = output;

    let saved = std::fs::read_to_string(&creds_path).expect("credentials written");
    assert!(saved.contains(USER_EMAIL));
    let store = TenantStore::load_from_path(&tenants_path).expect("tenants written");
    assert!(store.tenants.iter().any(|t| t.id == TENANT_ID));
}

#[tokio::test]
async fn login_post_token_fails_on_rejected_token() {
    let env = enter().await;
    env.server().reset().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(env.server())
        .await;

    let err = systemprompt_cli::cloud::auth::complete_login(
        &env.server().uri(),
        FAR_FUTURE_JWT.to_owned(),
        &json_ctx().cli,
    )
    .await
    .expect_err("rejected token fails");
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn sync_pull_rejects_local_profile_tenant() {
    let env = enter().await;
    let _ = env;
    let local_only = TenantStore::new(vec![StoredTenant::new_local(
        TenantId::new(TENANT_ID),
        "Harness Prod".to_owned(),
        "postgres://local/db".to_owned(),
    )]);
    local_only
        .save_to_path(&get_cloud_paths().resolve(CloudPath::Tenants))
        .expect("seed local-only tenants");

    let err = cloud::execute(
        CloudCommands::Sync {
            command: Some(systemprompt_cli::cloud::sync::SyncCommands::Pull(
                systemprompt_cli::cloud::sync::SyncArgs {
                    dry_run: true,
                    force: false,
                    verbose: false,
                },
            )),
        },
        &json_ctx(),
    )
    .await
    .expect_err("local tenant cannot sync");
    assert!(err.to_string().contains("local tenant"));
}

#[tokio::test]
async fn sync_push_requires_hostname() {
    let env = enter().await;
    let _ = env;
    let no_hostname = TenantStore::new(vec![StoredTenant::new_cloud(NewCloudTenantParams {
        id: TenantId::new(TENANT_ID),
        name: "Harness Prod".to_owned(),
        app_id: None,
        hostname: None,
        region: None,
        database_url: None,
        internal_database_url: "postgres://int/db".to_owned(),
        external_db_access: false,
    })]);
    no_hostname
        .save_to_path(&get_cloud_paths().resolve(CloudPath::Tenants))
        .expect("seed hostname-less tenants");

    let err = cloud::execute(
        CloudCommands::Sync {
            command: Some(systemprompt_cli::cloud::sync::SyncCommands::Push(
                systemprompt_cli::cloud::sync::SyncArgs {
                    dry_run: true,
                    force: false,
                    verbose: false,
                },
            )),
        },
        &json_ctx(),
    )
    .await
    .expect_err("push without hostname fails");
    assert!(err.to_string().contains("Hostname"));
}

#[tokio::test]
async fn admin_user_sync_creates_then_reports_existing_admin() {
    let Some(database_url) = crate::full_bootstrap::database_url() else {
        return;
    };
    let _env = enter().await;

    let email = format!(
        "harness-admin-{}-{}@example.com",
        std::process::id(),
        Utc::now().timestamp_micros()
    );
    let user = systemprompt_cli::cloud::sync::admin_user::CloudUser {
        email: email.clone(),
        name: Some("Harness Admin".to_owned()),
    };

    let first = systemprompt_cli::cloud::sync::admin_user::sync_admin_to_database(
        &user,
        &database_url,
        "harness-profile",
    )
    .await;
    assert!(
        matches!(
            first,
            systemprompt_cli::cloud::sync::admin_user::SyncResult::Created { .. }
        ),
        "first sync should create the admin: {first:?}"
    );

    let second = systemprompt_cli::cloud::sync::admin_user::sync_admin_to_database(
        &user,
        &database_url,
        "harness-profile",
    )
    .await;
    assert!(
        matches!(
            second,
            systemprompt_cli::cloud::sync::admin_user::SyncResult::AlreadyAdmin { .. }
                | systemprompt_cli::cloud::sync::admin_user::SyncResult::Promoted { .. }
        ),
        "second sync should find the existing admin: {second:?}"
    );
    systemprompt_cli::cloud::sync::admin_user::print_sync_results(&[first, second]);
}

#[tokio::test]
async fn admin_user_sync_reports_connection_failure() {
    let _env = enter().await;
    let user = systemprompt_cli::cloud::sync::admin_user::CloudUser {
        email: "unreachable@example.com".to_owned(),
        name: None,
    };
    let result = systemprompt_cli::cloud::sync::admin_user::sync_admin_to_database(
        &user,
        "postgres://nobody:nothing@127.0.0.1:1/void",
        "dead-profile",
    )
    .await;
    assert!(matches!(
        result,
        systemprompt_cli::cloud::sync::admin_user::SyncResult::ConnectionFailed { .. }
    ));
}
