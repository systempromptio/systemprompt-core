//! A spawned agent composes its `AgentHost` on the child environment alone:
//! it never reads the profile's secrets source, ignores a pepper, seed or
//! signing key placed in its environment, holds only the secrets it was
//! granted, and verifies bearers against the public key set it was handed.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_runtime::{AgentHost, HostContext};
use systemprompt_security::jwt::{AdminTokenParams, JwtService};
use systemprompt_security::keys::SigningKeys;
use systemprompt_test_fixtures::{
    FIXTURE_JWT_ISSUER, TestBootstrap, host_child_env, init_services_bootstrap, mint_admin_jwt,
    seed_fixture_system_admin, test_db_pool,
};
use systemprompt_traits::JwtProviderError;

const SERVICES: &str = r#"agents:
  host_agent:
    name: host_agent
    port: 9462
    endpoint: http://127.0.0.1:9462
    enabled: true
    card:
      protocolVersion: "0.3.0"
      displayName: Host Agent
      description: Agent composed on a host context in tests.
      version: "1.0.0"
    metadata: {}
    oauth:
      required: false
credentials:
  anthropic:
    kind: platform
    secret: anthropic_api_key
providers:
  anthropic:
    credential: anthropic
    endpoints:
      api:
        wire: anthropic
        url: http://127.0.0.1:1
models:
  claude-fixture-1:
    clients:
    - claude
    pricing:
      input_per_million: 3.0
      output_per_million: 15.0
      cache_read_per_million: 0.0
deployments:
  claude-fixture-1:
  - anthropic/api
"#;

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn bootstrap() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_services_bootstrap(SERVICES))
}

fn agent_child_env(bootstrap: &TestBootstrap) -> BTreeMap<String, String> {
    let mut env = host_child_env(bootstrap);
    env.insert(
        "SYSTEMPROMPT_CUSTOM_SECRETS".to_owned(),
        "anthropic_api_key".to_owned(),
    );
    env.insert("anthropic_api_key".to_owned(), "sk-agent-grant".to_owned());
    env
}

async fn compose(env: BTreeMap<String, String>) -> AgentHost {
    seed_fixture_system_admin(&test_db_pool().await)
        .await
        .expect("seed the system admin");
    let host = HostContext::boot(move |name| env.get(name).cloned())
        .await
        .expect("host boots");
    AgentHost::compose(host).await.expect("agent composes")
}

#[tokio::test]
async fn an_agent_composes_without_the_profile_secrets_source_and_ignores_boot_secrets_in_env() {
    let bootstrap = bootstrap();
    let profile = std::fs::read_to_string(&bootstrap.profile_path)
        .expect("profile")
        .replace(systemprompt_test_fixtures::FIXTURE_SECRETS_BLOCK, "");
    let absent = bootstrap
        .profile_path
        .with_file_name("agent-absent-secrets.json");
    let file_secrets = bootstrap
        .profile_path
        .with_file_name("agent-file-secrets.yaml");
    std::fs::write(
        &file_secrets,
        format!(
            "{profile}secrets:\n  source: file\n  secrets_path: {}\n",
            absent.display()
        ),
    )
    .expect("write profile");
    let mut env = agent_child_env(bootstrap);
    env.insert(
        "SYSTEMPROMPT_PROFILE".to_owned(),
        file_secrets.display().to_string(),
    );
    env.insert("OAUTH_AT_REST_PEPPER".to_owned(), "p".repeat(48));
    env.insert("MANIFEST_SIGNING_SECRET_SEED".to_owned(), "00".repeat(32));
    env.insert(
        "SIGNING_KEY_PEM".to_owned(),
        "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n".to_owned(),
    );

    let agent = compose(env).await;

    assert!(!absent.exists(), "the secrets file was never created");
    let host = agent.host();
    assert_eq!(
        host.granted_secrets().granted_names(),
        vec!["anthropic_api_key".to_owned()]
    );
    let secrets = host.secrets();
    assert!(secrets.secret("oauth_at_rest_pepper").is_none());
    assert!(secrets.secret("signing_key_pem").is_none());
    assert!(secrets.manifest_signing_secret_seed().is_err());
    assert!(secrets.encryption_master_key().is_err());
}

#[tokio::test]
async fn the_agent_verifies_bearers_against_the_key_set_it_was_handed() {
    let agent = compose(agent_child_env(bootstrap())).await;
    let auth = agent.agent_auth();
    let user = UserId::new(uuid::Uuid::new_v4().to_string());

    let token = mint_admin_jwt(&user, "agent-host@test.invalid", FIXTURE_JWT_ISSUER);
    let claims = auth
        .jwt_provider
        .validate_token(token.as_str())
        .expect("a token from the deployment key verifies");
    assert_eq!(claims.subject, user);

    let foreign = SigningKeys::new(systemprompt_test_fixtures::keys::test_key(1), &[0; 32])
        .expect("foreign keys");
    let forged = JwtService::generate_admin_token(
        &foreign,
        &AdminTokenParams {
            user_id: &user,
            session_id: &SessionId::generate(),
            email: "agent-host@test.invalid",
            issuer: FIXTURE_JWT_ISSUER,
            duration: chrono::Duration::hours(1),
            client_id: None,
        },
    )
    .expect("mint with a foreign key");
    let err = auth
        .jwt_provider
        .validate_token(forged.as_str())
        .expect_err("a key outside the set never verifies");
    assert!(matches!(err, JwtProviderError::InvalidToken), "{err:?}");
}
