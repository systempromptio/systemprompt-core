use systemprompt_config::{SecretsBootstrapError, SecretsProvider, VaultError, VaultKvProvider};
use systemprompt_models::profile::VaultSecretsConfig;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::vault_fixture as fx;

const KV_PATH: &str = "/v1/secret/data/systemprompt/prod";

fn provider(server: &MockServer, mutate: impl FnOnce(&mut VaultSecretsConfig)) -> VaultKvProvider {
    let mut cfg = fx::config(&server.uri(), fx::token_auth());
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

#[tokio::test]
async fn a_kv_document_becomes_secrets_with_nulls_stripped() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .and(header("X-Vault-Token", "root-token"))
        .and(header("X-Vault-Request", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(fx::secrets_document())))
        .expect(1)
        .mount(&server)
        .await;

    let document = provider(&server, |_c| {}).fetch().await.unwrap();
    assert!(document.key_names().contains(&"database_url".to_owned()));

    let secrets = document.into_secrets().unwrap();
    assert_eq!(secrets.oauth_at_rest_pepper, fx::PEPPER);
    assert_eq!(secrets.database_url, fx::DB_URL);
    assert!(secrets.gemini.is_none());
    assert!(secrets.custom.is_empty());
}

#[tokio::test]
async fn a_missing_document_reports_the_mount_and_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"errors": []})))
        .expect(1)
        .mount(&server)
        .await;

    let err = vault_error(provider(&server, |_c| {}).fetch().await.unwrap_err());
    assert!(matches!(
        err,
        VaultError::NotFound { ref mount, ref path }
            if mount == "secret" && path == "systemprompt/prod"
    ));
}

#[tokio::test]
async fn a_denied_read_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(
                serde_json::json!({"errors": ["1 error occurred: permission denied"]}),
            ),
        )
        .expect(1)
        .mount(&server)
        .await;

    let err = vault_error(provider(&server, |_c| {}).fetch().await.unwrap_err());
    let rendered = err.to_string();
    assert!(matches!(err, VaultError::Forbidden { .. }));
    assert!(rendered.contains("permission denied"));
}

#[tokio::test]
async fn transient_server_errors_are_retried_until_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(fx::secrets_document())))
        .expect(1)
        .mount(&server)
        .await;

    let document = provider(&server, |_c| {}).fetch().await.unwrap();
    assert!(
        document
            .key_names()
            .contains(&"oauth_at_rest_pepper".to_owned())
    );
}

#[tokio::test]
async fn a_permanently_unavailable_vault_exhausts_its_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(503))
        .expect(3)
        .mount(&server)
        .await;

    let err = vault_error(provider(&server, |_c| {}).fetch().await.unwrap_err());
    assert!(matches!(err, VaultError::Exhausted { attempts: 3, .. }));
}

#[tokio::test]
async fn a_malformed_body_is_rejected_as_malformed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_string("{not json"))
        .expect(1)
        .mount(&server)
        .await;

    let err = vault_error(provider(&server, |_c| {}).fetch().await.unwrap_err());
    assert!(matches!(err, VaultError::Malformed { .. }));
}

#[tokio::test]
async fn the_namespace_header_is_sent_when_configured() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .and(header("X-Vault-Namespace", "team-a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(fx::secrets_document())))
        .expect(1)
        .mount(&server)
        .await;

    provider(&server, |c| c.namespace = Some("team-a".to_owned()))
        .fetch()
        .await
        .unwrap();
}

#[tokio::test]
async fn a_redirect_is_not_followed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "https://evil.example/"))
        .expect(1)
        .mount(&server)
        .await;

    let err = vault_error(provider(&server, |_c| {}).fetch().await.unwrap_err());
    assert!(matches!(err, VaultError::Http { status: 302, .. }));
}

#[tokio::test]
async fn a_key_override_is_merged_over_the_base_document() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(fx::secrets_document())))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/secret/data/shared/identity"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(
            serde_json::json!({ "seed": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBA=" }),
        )))
        .expect(1)
        .mount(&server)
        .await;

    let document = provider(&server, |c| {
        c.keys.insert(
            "manifest_signing_secret_seed".to_owned(),
            fx::key_override("shared/identity", "seed"),
        );
    })
    .fetch()
    .await
    .unwrap();

    let secrets = document.into_secrets().unwrap();
    assert_eq!(
        secrets.manifest_signing_secret_seed.as_deref(),
        Some("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBA=")
    );
}
