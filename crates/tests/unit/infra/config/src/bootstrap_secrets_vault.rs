use systemprompt_config::{ConfigError, ProfileBootstrap, SecretsBootstrap, SecretsBootstrapError};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::{fixture, vault_fixture as fx};

const KV_PATH: &str = "/v1/secret/data/systemprompt/prod";

fn vault_secrets_section(address: &str) -> String {
    format!(
        "secrets:\n  source: vault\n  validation: strict\n  vault:\n    address: {address}\n    \
         mount: secret\n    path: systemprompt/prod\n    retries: 2\n    auth:\n      method: \
         token\n      token_env: VAULT_TOKEN\n"
    )
}

fn document() -> serde_json::Value {
    serde_json::json!({
        "oauth_at_rest_pepper": fixture::PEPPER,
        "database_url": fixture::DB_URL,
        "manifest_signing_secret_seed": fixture::SEED,
    })
}

#[tokio::test]
async fn init_loads_the_secrets_document_from_vault() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(document())))
        .expect(1)
        .mount(&server)
        .await;

    fx::set_env("VAULT_TOKEN", "root-token");
    let profile = fixture::write_tree(&vault_secrets_section(&server.uri()), None);
    ProfileBootstrap::init_from_path(&profile.profile_path).unwrap();

    let secrets = SecretsBootstrap::init().await.unwrap();

    assert_eq!(secrets.oauth_at_rest_pepper, fixture::PEPPER);
    assert_eq!(secrets.database_url, fixture::DB_URL);
    assert!(SecretsBootstrap::is_initialized());
}

#[tokio::test]
async fn an_unreachable_vault_fails_the_boot_even_with_a_valid_pepper_in_the_environment() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    fx::set_env("VAULT_TOKEN", "root-token");
    fx::set_env("OAUTH_AT_REST_PEPPER", fixture::PEPPER);
    fx::set_env("DATABASE_URL", fixture::DB_URL);
    fx::set_env("MANIFEST_SIGNING_SECRET_SEED", fixture::SEED);

    let profile = fixture::write_tree(&vault_secrets_section(&server.uri()), None);
    ProfileBootstrap::init_from_path(&profile.profile_path).unwrap();

    let err = SecretsBootstrap::init().await.unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::Vault(_))
    ));
    assert!(!SecretsBootstrap::is_initialized());
}
