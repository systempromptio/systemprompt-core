use base64::prelude::*;
use chrono::Utc;
use systemprompt_cloud::OAuthTemplates;
use systemprompt_cloud::error::CloudError;
use systemprompt_cloud::operator::{
    OperatorCredentials, OperatorLoginRequest, exchange_for_tenant, run_operator_login,
};
use systemprompt_identifiers::{ClientId, CloudAuthToken, Email};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn token_expiring_at(exp: i64) -> String {
    let header = BASE64_URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    let claims = BASE64_URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{exp}}}"#));
    format!("{header}.{claims}.sig")
}

fn credentials(id_token: &str) -> OperatorCredentials {
    OperatorCredentials {
        issuer: "https://issuer.example".to_owned(),
        client_id: ClientId::new("cli"),
        id_token: CloudAuthToken::new(id_token),
        email: Some(Email::try_new("op@example.com").unwrap()),
        authenticated_at: Utc::now(),
    }
}

fn templates() -> OAuthTemplates {
    OAuthTemplates {
        success_html: "ok",
        error_html: "err",
    }
}

fn request(issuer: String) -> OperatorLoginRequest {
    OperatorLoginRequest {
        issuer,
        client_id: ClientId::new("cli"),
        scopes: vec!["openid".to_owned()],
    }
}

#[test]
fn a_saved_credential_loads_back_and_validates_when_unexpired() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("operator.json");
    let token = token_expiring_at(Utc::now().timestamp() + 3600);
    credentials(&token).save_to_path(&file).unwrap();
    let loaded = OperatorCredentials::load_valid(&file).unwrap();
    assert_eq!(loaded.id_token.as_str(), token);
    assert!(!loaded.is_expired());
    assert_eq!(loaded.email.unwrap().as_str(), "op@example.com");
}

#[test]
fn an_expired_credential_is_not_authenticated() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("operator.json");
    credentials(&token_expiring_at(Utc::now().timestamp() - 10))
        .save_to_path(&file)
        .unwrap();
    let error = OperatorCredentials::load_valid(&file).unwrap_err();
    assert!(matches!(error, CloudError::OperatorNotAuthenticated));
}

#[test]
fn a_missing_credential_file_is_not_authenticated() {
    let dir = tempfile::tempdir().unwrap();
    let error = OperatorCredentials::load_from_path(&dir.path().join("none.json")).unwrap_err();
    assert!(matches!(error, CloudError::OperatorNotAuthenticated));
}

#[test]
fn a_corrupt_credential_file_is_reported_corrupted() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("operator.json");
    std::fs::write(&file, "{not json").unwrap();
    let error = OperatorCredentials::load_from_path(&file).unwrap_err();
    assert!(matches!(error, CloudError::CredentialsCorrupted { .. }));
}

#[test]
fn delete_removes_the_file_and_tolerates_absence() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("operator.json");
    credentials("a.b.c").save_to_path(&file).unwrap();
    OperatorCredentials::delete_from_path(&file).unwrap();
    assert!(!file.exists());
    OperatorCredentials::delete_from_path(&file).unwrap();
}

#[tokio::test]
async fn exchange_for_tenant_trades_the_id_token_for_a_tenant_session() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .and(body_string_contains("subject_token=id-tok"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "tenant-at",
            "expires_in": 600,
        })))
        .expect(1)
        .mount(&server)
        .await;
    let session = exchange_for_tenant(&server.uri(), &credentials("id-tok"))
        .await
        .unwrap();
    assert_eq!(session.access_token, "tenant-at");
}

#[tokio::test]
async fn exchange_for_tenant_reports_a_refused_exchange() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let error = exchange_for_tenant(&server.uri(), &credentials("id-tok"))
        .await
        .unwrap_err();
    assert!(matches!(error, CloudError::Unauthorized));
}

async fn mount_discovery(server: &MockServer, status: u16, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn login_refuses_a_failed_discovery() {
    let server = MockServer::start().await;
    mount_discovery(&server, 404, serde_json::json!({})).await;
    let error = run_operator_login(&request(server.uri()), templates())
        .await
        .unwrap_err();
    assert!(
        matches!(&error, CloudError::OperatorAuth { message } if message.contains("404")),
        "{error:?}"
    );
}

#[tokio::test]
async fn login_refuses_a_discovery_naming_another_issuer() {
    let server = MockServer::start().await;
    let uri = server.uri();
    mount_discovery(
        &server,
        200,
        serde_json::json!({
            "issuer": "https://elsewhere.example",
            "authorization_endpoint": format!("{uri}/authorize"),
            "token_endpoint": format!("{uri}/token"),
        }),
    )
    .await;
    let error = run_operator_login(&request(format!("{uri}/")), templates())
        .await
        .unwrap_err();
    assert!(
        matches!(&error, CloudError::OperatorAuth { message } if message.contains("elsewhere")),
        "{error:?}"
    );
}

#[tokio::test]
async fn login_refuses_a_plain_http_endpoint_off_loopback() {
    let server = MockServer::start().await;
    let uri = server.uri();
    mount_discovery(
        &server,
        200,
        serde_json::json!({
            "issuer": uri,
            "authorization_endpoint": format!("{uri}/authorize"),
            "token_endpoint": "http://idp.example.com/token",
        }),
    )
    .await;
    let error = run_operator_login(&request(uri), templates())
        .await
        .unwrap_err();
    assert!(
        matches!(&error, CloudError::OperatorEndpoint { endpoint, .. } if endpoint == "http://idp.example.com/token"),
        "{error:?}"
    );
}
