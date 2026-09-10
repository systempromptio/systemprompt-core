use systemprompt_config::{SecretsBootstrapError, SecretsProvider, VaultError, VaultKvProvider};
use systemprompt_models::profile::VaultSecretsConfig;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::vault_fixture as fx;

const KV_PATH: &str = "/v1/secret/data/systemprompt/prod";

fn provider(address: &str, mutate: impl FnOnce(&mut VaultSecretsConfig)) -> VaultKvProvider {
    let mut cfg = fx::config(address, fx::token_auth());
    mutate(&mut cfg);
    VaultKvProvider::from_config(&cfg, |_name| Some("root-token".to_owned()))
        .expect("provider builds against a loopback address")
}

fn vault_error(err: SecretsBootstrapError) -> VaultError {
    match err {
        SecretsBootstrapError::Vault(e) => e,
        other => panic!("expected a vault error, got {other}"),
    }
}

#[test]
fn an_unreadable_ca_certificate_names_the_path_it_tried() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("absent-ca.pem");
    let mut cfg = fx::config("https://127.0.0.1:8200", fx::token_auth());
    cfg.ca_cert_path = Some(missing.display().to_string());

    let err = VaultKvProvider::from_config(&cfg, |_name| None).unwrap_err();

    assert!(matches!(
        err,
        VaultError::CaCertificate { ref path, .. } if path == &missing.display().to_string()
    ));
}

#[tokio::test]
async fn a_vault_that_is_not_listening_is_reported_as_unreachable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);

    let err = vault_error(
        provider(&address, |c| c.retries = 2)
            .fetch()
            .await
            .unwrap_err(),
    );

    assert!(
        matches!(err, VaultError::Exhausted { attempts: 2, .. }),
        "got {err}"
    );
}

#[tokio::test]
async fn a_rate_limited_read_is_retried_and_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(fx::secrets_document())))
        .expect(1)
        .mount(&server)
        .await;

    let document = provider(&server.uri(), |_c| {}).fetch().await.unwrap();

    assert!(document.key_names().contains(&"database_url".to_owned()));
}

#[tokio::test]
async fn a_zero_retry_configuration_still_makes_one_attempt() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&server)
        .await;

    let err = vault_error(
        provider(&server.uri(), |c| c.retries = 0)
            .fetch()
            .await
            .unwrap_err(),
    );

    assert!(matches!(
        err,
        VaultError::Exhausted {
            attempts: 1,
            ref message
        } if message == "HTTP 500"
    ));
}

#[tokio::test]
async fn a_read_that_outlives_the_timeout_is_retried_rather_than_hanging() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(2))
                .set_body_json(fx::kv_body(fx::secrets_document())),
        )
        .mount(&server)
        .await;

    let err = vault_error(
        provider(&server.uri(), |c| {
            c.retries = 2;
            c.timeout_secs = 1;
        })
        .fetch()
        .await
        .unwrap_err(),
    );

    assert!(matches!(
        err,
        VaultError::Exhausted {
            attempts: 2,
            ref message
        } if message == "request timed out"
    ));
}
