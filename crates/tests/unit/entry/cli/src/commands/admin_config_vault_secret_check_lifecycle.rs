//! Vault-backed secret checks fail safely and recover against the configured
//! source.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use systemprompt_cli::admin::config::secret_check;
use systemprompt_cli::cloud::doctor::{CheckStatus, vault_checks};
use systemprompt_config::ProfileBootstrap;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const OWNED_COVERAGE_VAULT_TOKEN: &str = "owned-vault-token-never-rendered";
const DATABASE_SECRET: &str = "postgres://owned:password@127.0.0.1:1/private";
const PEPPER_SECRET: &str = "owned-pepper-value-that-is-long-enough";

#[derive(Clone)]
struct RepairableVault {
    failing: Arc<AtomicBool>,
}

impl Respond for RepairableVault {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        if self.failing.load(Ordering::SeqCst) {
            ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "errors": ["owned vault is temporarily unavailable"]
            }))
        } else {
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"data": {
                    "database_url": DATABASE_SECRET,
                    "oauth_at_rest_pepper": PEPPER_SECRET,
                    "optional_owned_key": "optional-value-never-rendered"
                }}
            }))
        }
    }
}

fn profile_yaml(root: &std::path::Path, vault_address: &str) -> String {
    let system = root.join("system");
    let services = root.join("services");
    let bin = root.join("bin");
    let storage = root.join("storage");
    let web = system.join("web");
    for path in [&system, &services, &bin, &storage, &web] {
        std::fs::create_dir_all(path).expect("create owned profile path");
    }
    format!(
        r#"name: vaultcheck
display_name: Vault Check Fixture
target: local
site:
  name: vaultcheck
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
  cors_allowed_origins: [http://127.0.0.1]
  content_negotiation:
    enabled: false
    markdown_suffix: .md
  security_headers:
    enabled: true
    hsts: max-age=63072000
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
  jwt_issuer: https://issuer.example.invalid
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
  jwt_audiences: [api]
  allowed_resource_audiences: [hook]
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
system_admin:
  username: vaultadmin
  email: vaultadmin@example.invalid
runtime:
  environment: development
  log_level: quiet
  output_format: json
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
secrets:
  source: vault
  validation: strict
  vault:
    address: {vault_address}
    mount: secret
    path: systemprompt/owned
    timeout_secs: 2
    retries: 0
    auth:
      method: token
      token_env: OWNED_COVERAGE_VAULT_TOKEN
"#,
        system = system.display(),
        services = services.display(),
        bin = bin.display(),
        web = web.display(),
        storage = storage.display(),
    )
}

#[tokio::test]
async fn vault_secret_check_reports_transport_failure_then_recovers_without_exposing_values() {
    let server = MockServer::start().await;
    let failing = Arc::new(AtomicBool::new(true));
    Mock::given(method("GET"))
        .and(path("/v1/secret/data/systemprompt/owned"))
        .and(header("x-vault-token", OWNED_COVERAGE_VAULT_TOKEN))
        .respond_with(RepairableVault {
            failing: Arc::clone(&failing),
        })
        .expect(4)
        .mount(&server)
        .await;
    let project = tempfile::tempdir().expect("owned Vault profile");
    let profile_path = project
        .path()
        .join(".systemprompt/profiles/vaultcheck/profile.yaml");
    std::fs::create_dir_all(profile_path.parent().expect("profile directory"))
        .expect("create profile directory");
    std::fs::write(&profile_path, profile_yaml(project.path(), &server.uri()))
        .expect("write Vault profile");
    // SAFETY: nextest runs this test in its own process and profile initialization
    // has not run.
    unsafe { std::env::set_var("OWNED_COVERAGE_VAULT_TOKEN", OWNED_COVERAGE_VAULT_TOKEN) };
    ProfileBootstrap::init_from_path(&profile_path).expect("initialize Vault profile");
    let previous = std::env::current_dir().expect("current directory");
    std::env::set_current_dir(project.path()).expect("enter owned Vault project");
    struct CwdGuard(std::path::PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }
    let _cwd = CwdGuard(previous);
    let profile = ProfileBootstrap::get().expect("Vault profile");
    let vault = profile
        .secrets
        .as_ref()
        .and_then(|secrets| secrets.vault.as_ref())
        .expect("Vault configuration");

    let error = secret_check::run()
        .await
        .expect_err("HTTP 500 must fail the secret check");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("Vault document could not be read"),
        "{rendered}"
    );
    assert!(rendered.contains("500"), "{rendered}");
    for secret in [
        OWNED_COVERAGE_VAULT_TOKEN,
        DATABASE_SECRET,
        PEPPER_SECRET,
        "optional-value-never-rendered",
    ] {
        assert!(
            !rendered.contains(secret),
            "secret leaked in failure diagnostic"
        );
    }
    let (failed_check, failed_values) = vault_checks::check_vault_document(vault).await;
    assert_eq!(failed_check.status, CheckStatus::Fail);
    assert!(
        failed_check.detail.contains("500"),
        "{}",
        failed_check.detail
    );
    assert!(failed_values.is_empty());

    failing.store(false, Ordering::SeqCst);
    let report = secret_check::run().await.expect("repaired Vault check");
    assert_eq!(report.source, "vault");
    assert!(
        report.detail.contains("secret/systemprompt/owned"),
        "{}",
        report.detail
    );
    assert_eq!(
        report.keys,
        ["database_url", "oauth_at_rest_pepper", "optional_owned_key"]
    );
    assert!(report.missing_required.is_empty());
    let serialized = serde_json::to_string(&report).expect("serialize public report");
    for secret in [
        OWNED_COVERAGE_VAULT_TOKEN,
        DATABASE_SECRET,
        PEPPER_SECRET,
        "optional-value-never-rendered",
    ] {
        assert!(
            !serialized.contains(secret),
            "secret leaked in successful report"
        );
    }
    let (healthy_check, healthy_values) = vault_checks::check_vault_document(vault).await;
    assert_eq!(healthy_check.status, CheckStatus::Pass);
    assert_eq!(
        healthy_check.detail,
        "document secret/systemprompt/owned readable via token, 3 keys"
    );
    assert_eq!(
        healthy_values.get("database_url").map(String::as_str),
        Some(DATABASE_SECRET)
    );
    assert_eq!(
        healthy_values
            .get("oauth_at_rest_pepper")
            .map(String::as_str),
        Some(PEPPER_SECRET)
    );
}
