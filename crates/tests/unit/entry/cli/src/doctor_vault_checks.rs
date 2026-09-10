//! `cloud doctor` preflight for a Vault-backed profile.
//!
//! The address check is pure. The document check talks to a stubbed KV v2
//! endpoint so that a readable document, an auth failure, and an unparsable
//! document are all distinguishable before an image is built.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use serde_json::json;
use systemprompt_cli::cloud::doctor::CheckStatus;
use systemprompt_cli::cloud::doctor::vault_checks::{check_vault_address, check_vault_document};
use systemprompt_models::profile::{VaultAuth, VaultSecretsConfig};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PEPPER: &str = "0123456789abcdef0123456789abcdef0123456789";

fn config(address: &str, token_env: &str) -> VaultSecretsConfig {
    VaultSecretsConfig {
        address: address.to_owned(),
        mount: "secret".to_owned(),
        path: "systemprompt/prod".to_owned(),
        namespace: None,
        auth: VaultAuth::Token {
            token_env: token_env.to_owned(),
            token_file: None,
        },
        keys: BTreeMap::new(),
        ca_cert_path: None,
        timeout_secs: 5,
        retries: 1,
    }
}

#[test]
fn a_plain_http_address_outside_loopback_is_refused() {
    let result = check_vault_address(&config("http://vault.example.test", "VAULT_TOKEN"));
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("https"), "{}", result.detail);
}

#[test]
fn a_non_url_address_is_refused() {
    let result = check_vault_address(&config("vault.example.test:8200", "VAULT_TOKEN"));
    assert_eq!(result.status, CheckStatus::Fail);
}

#[test]
fn an_https_address_passes() {
    let result = check_vault_address(&config("https://vault.example.test", "VAULT_TOKEN"));
    assert_eq!(result.status, CheckStatus::Pass);
}

#[test]
fn a_loopback_http_address_passes() {
    let result = check_vault_address(&config("http://127.0.0.1:8200", "VAULT_TOKEN"));
    assert_eq!(result.status, CheckStatus::Pass);
}

#[tokio::test]
async fn a_readable_document_reports_its_key_count() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/secret/data/systemprompt/prod"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "data": { "oauth_at_rest_pepper": PEPPER, "database_url": "postgres://x/y" },
                "metadata": { "version": 3 }
            }
        })))
        .mount(&server)
        .await;

    let env = "SP_TEST_VAULT_TOKEN_OK";
    unsafe { std::env::set_var(env, "hvs.test") };
    let (result, values) = check_vault_document(&config(&server.uri(), env)).await;
    unsafe { std::env::remove_var(env) };

    assert_eq!(result.status, CheckStatus::Pass, "{}", result.detail);
    assert!(
        result.detail.contains("secret/systemprompt/prod"),
        "{}",
        result.detail
    );
    assert!(result.detail.contains("via token"), "{}", result.detail);
    assert!(result.detail.contains("2 keys"), "{}", result.detail);
    assert_eq!(
        values.get("database_url").map(String::as_str),
        Some("postgres://x/y")
    );
}

#[tokio::test]
async fn a_rejected_token_fails_the_preflight() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/secret/data/systemprompt/prod"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "errors": ["denied"] })))
        .mount(&server)
        .await;

    let env = "SP_TEST_VAULT_TOKEN_DENIED";
    unsafe { std::env::set_var(env, "hvs.test") };
    let (result, values) = check_vault_document(&config(&server.uri(), env)).await;
    unsafe { std::env::remove_var(env) };

    assert_eq!(result.status, CheckStatus::Fail);
    assert!(values.is_empty());
    assert!(
        !result.detail.contains("hvs.test"),
        "the token leaked into the report: {}",
        result.detail
    );
}

#[tokio::test]
async fn a_document_missing_required_keys_fails_but_still_reports_the_keys_it_has() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/secret/data/systemprompt/prod"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "data": { "anthropic": "sk-test" },
                "metadata": { "version": 1 }
            }
        })))
        .mount(&server)
        .await;

    let env = "SP_TEST_VAULT_TOKEN_PARTIAL";
    unsafe { std::env::set_var(env, "hvs.test") };
    let (result, values) = check_vault_document(&config(&server.uri(), env)).await;
    unsafe { std::env::remove_var(env) };

    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("1 keys"), "{}", result.detail);
    assert!(values.contains_key("anthropic"));
    assert!(
        !result.detail.contains("sk-test"),
        "a value leaked: {}",
        result.detail
    );
}
