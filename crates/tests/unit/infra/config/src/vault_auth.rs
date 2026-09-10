use std::io::Write;

use systemprompt_config::{SecretsBootstrapError, SecretsProvider, VaultError, VaultKvProvider};
use systemprompt_models::profile::VaultAuth;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::vault_fixture as fx;

const KV_PATH: &str = "/v1/secret/data/systemprompt/prod";

fn vault_error(err: SecretsBootstrapError) -> VaultError {
    match err {
        SecretsBootstrapError::Vault(e) => e,
        other => panic!("expected a vault error, got {other}"),
    }
}

async fn mount_login(server: &MockServer, login_path: &str, body: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path(login_path.to_owned()))
        .and(body_json(body))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "auth": { "client_token": "minted-token", "lease_duration": 3600 }
        })))
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_document(server: &MockServer, expected_token: &str) {
    Mock::given(method("GET"))
        .and(path(KV_PATH))
        .and(header("X-Vault-Token", expected_token))
        .respond_with(ResponseTemplate::new(200).set_body_json(fx::kv_body(fx::secrets_document())))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn approle_login_posts_both_ids_and_the_minted_token_is_reused() {
    let server = MockServer::start().await;
    mount_login(
        &server,
        "/v1/auth/approle/login",
        serde_json::json!({ "role_id": "role-1", "secret_id": "secret-1" }),
    )
    .await;
    mount_document(&server, "minted-token").await;

    let cfg = fx::config(
        &server.uri(),
        VaultAuth::AppRole {
            role_id_env: "VAULT_ROLE_ID".to_owned(),
            secret_id_env: "VAULT_SECRET_ID".to_owned(),
            mount: "approle".to_owned(),
        },
    );
    let provider = VaultKvProvider::from_config(&cfg, |name| match name {
        "VAULT_ROLE_ID" => Some("role-1".to_owned()),
        "VAULT_SECRET_ID" => Some("secret-1".to_owned()),
        _ => None,
    })
    .unwrap();

    provider.fetch().await.unwrap();
}

#[tokio::test]
async fn kubernetes_login_reads_the_service_account_jwt_from_disk() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let jwt_path = dir.path().join("token");
    std::fs::write(&jwt_path, "header.payload.signature\n").unwrap();

    mount_login(
        &server,
        "/v1/auth/kubernetes/login",
        serde_json::json!({ "role": "systemprompt", "jwt": "header.payload.signature" }),
    )
    .await;
    mount_document(&server, "minted-token").await;

    let cfg = fx::config(
        &server.uri(),
        VaultAuth::Kubernetes {
            role: "systemprompt".to_owned(),
            jwt_path: jwt_path.display().to_string(),
            mount: "kubernetes".to_owned(),
        },
    );
    let provider = VaultKvProvider::from_config(&cfg, |_name| None).unwrap();

    provider.fetch().await.unwrap();
}

#[tokio::test]
async fn a_token_from_the_environment_is_sent_verbatim() {
    let server = MockServer::start().await;
    mount_document(&server, "env-token").await;

    let cfg = fx::config(&server.uri(), fx::token_auth());
    let provider =
        VaultKvProvider::from_config(&cfg, |_name| Some("env-token".to_owned())).unwrap();

    provider.fetch().await.unwrap();
}

#[tokio::test]
async fn a_token_file_is_trimmed_of_its_trailing_newline() {
    let server = MockServer::start().await;
    mount_document(&server, "file-token").await;

    let dir = tempfile::tempdir().unwrap();
    let token_path = dir.path().join("vault-token");
    let mut file = std::fs::File::create(&token_path).unwrap();
    file.write_all(b"file-token\n").unwrap();
    drop(file);

    let cfg = fx::config(
        &server.uri(),
        VaultAuth::Token {
            token_env: "VAULT_TOKEN".to_owned(),
            token_file: Some(token_path.display().to_string()),
        },
    );
    let provider = VaultKvProvider::from_config(&cfg, |_name| None).unwrap();

    provider.fetch().await.unwrap();
}

#[tokio::test]
async fn a_missing_credential_names_the_variable_it_wanted() {
    let server = MockServer::start().await;
    let cfg = fx::config(
        &server.uri(),
        VaultAuth::AppRole {
            role_id_env: "VAULT_ROLE_ID".to_owned(),
            secret_id_env: "VAULT_SECRET_ID".to_owned(),
            mount: "approle".to_owned(),
        },
    );
    let provider = VaultKvProvider::from_config(&cfg, |_name| None).unwrap();

    let err = vault_error(provider.fetch().await.unwrap_err());
    assert!(matches!(
        err,
        VaultError::MissingCredential { ref name } if name == "VAULT_ROLE_ID"
    ));
}

#[tokio::test]
async fn a_rejected_login_reports_the_method_and_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/approle/login"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(serde_json::json!({"errors": ["invalid role or secret id"]})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let cfg = fx::config(
        &server.uri(),
        VaultAuth::AppRole {
            role_id_env: "VAULT_ROLE_ID".to_owned(),
            secret_id_env: "VAULT_SECRET_ID".to_owned(),
            mount: "approle".to_owned(),
        },
    );
    let provider = VaultKvProvider::from_config(&cfg, |_name| Some("value".to_owned())).unwrap();

    let err = vault_error(provider.fetch().await.unwrap_err());
    assert!(matches!(
        err,
        VaultError::Auth {
            method: "approle",
            status: 400,
            ..
        }
    ));
    assert!(err.to_string().contains("invalid role or secret id"));
}
