use systemprompt_config::{
    SecretsBootstrapError, SecretsDocument, SecretsProvider, VaultKvProvider,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::vault_fixture as fx;

const KV_PATH: &str = "/v1/secret/data/systemprompt/prod";

fn object(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => map,
        other => panic!("expected a JSON object, got {other}"),
    }
}

#[test]
fn a_default_document_holds_no_keys() {
    let document = SecretsDocument::default();

    assert!(document.is_empty());
    assert!(document.key_names().is_empty());
}

#[test]
fn key_names_are_sorted_and_a_merge_replaces_an_existing_field() {
    let mut document = SecretsDocument::new(object(serde_json::json!({
        "zeta": "z",
        "alpha": "a",
        "database_url": "postgresql://old",
    })));
    document.merge_field("database_url", serde_json::json!(fx::DB_URL));
    document.merge_field("middle".to_owned(), serde_json::json!("m"));

    assert!(!document.is_empty());
    assert_eq!(
        document.key_names(),
        vec![
            "alpha".to_owned(),
            "database_url".to_owned(),
            "middle".to_owned(),
            "zeta".to_owned()
        ]
    );
}

#[test]
fn a_document_missing_the_required_pepper_fails_to_become_secrets() {
    let document = SecretsDocument::new(object(serde_json::json!({
        "database_url": fx::DB_URL,
    })));

    let err = document.into_secrets().unwrap_err();

    assert!(matches!(
        err,
        SecretsBootstrapError::InvalidSecretsFile { .. }
    ));
}

#[test]
fn a_document_whose_field_has_the_wrong_type_fails_to_become_secrets() {
    let document = SecretsDocument::new(object(serde_json::json!({
        "oauth_at_rest_pepper": 17,
        "database_url": fx::DB_URL,
    })));

    assert!(matches!(
        document.into_secrets().unwrap_err(),
        SecretsBootstrapError::InvalidSecretsFile { .. }
    ));
}

#[tokio::test]
async fn the_provider_description_names_the_address_mount_path_and_auth_method() {
    let cfg = fx::config("https://vault.example.com", fx::token_auth());
    let provider = VaultKvProvider::from_config(&cfg, |_name| None).unwrap();

    assert_eq!(
        provider.describe(),
        "vault https://vault.example.com (secret/systemprompt/prod, auth token)"
    );
}

#[test]
fn the_debug_rendering_carries_the_coordinates_and_never_a_token() {
    let cfg = fx::config("https://vault.example.com", fx::token_auth());
    let provider =
        VaultKvProvider::from_config(&cfg, |_name| Some("super-secret".to_owned())).unwrap();

    let rendered = format!("{provider:?}");

    assert!(rendered.contains("vault.example.com"));
    assert!(rendered.contains("systemprompt/prod"));
    assert!(rendered.contains("token"));
    assert!(!rendered.contains("super-secret"));
}

#[tokio::test]
async fn a_key_override_pointing_at_an_absent_field_names_the_override_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(fx::secrets_document())))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/secret/data/shared/identity"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fx::kv_body(serde_json::json!({ "other_field": "value" }))),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut cfg = fx::config(&server.uri(), fx::token_auth());
    cfg.keys.insert(
        "manifest_signing_secret_seed".to_owned(),
        fx::key_override("shared/identity", "seed"),
    );
    let provider =
        VaultKvProvider::from_config(&cfg, |_name| Some("root-token".to_owned())).unwrap();

    let err = provider.fetch().await.unwrap_err();
    let rendered = err.to_string();

    assert!(rendered.contains("has no field 'seed'"));
    assert!(rendered.contains("manifest_signing_secret_seed"));
}

#[tokio::test]
async fn a_key_override_failure_on_the_second_read_does_not_yield_a_partial_document() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(fx::secrets_document())))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/secret/data/shared/identity"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({"errors": []})))
        .expect(1)
        .mount(&server)
        .await;

    let mut cfg = fx::config(&server.uri(), fx::token_auth());
    cfg.keys.insert(
        "database_url".to_owned(),
        fx::key_override("shared/identity", "url"),
    );
    let provider =
        VaultKvProvider::from_config(&cfg, |_name| Some("root-token".to_owned())).unwrap();

    assert!(provider.fetch().await.is_err());
}
