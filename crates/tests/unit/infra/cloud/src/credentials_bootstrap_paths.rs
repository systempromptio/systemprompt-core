//! File-backed (non-Fly) `CredentialsBootstrap` behaviour: the validation-TTL
//! skip, timestamp persistence after a live validation, `reload`, and the
//! expired / soon-expiring token branches. Each test rebinds the process cwd
//! to a fresh project root; nextest's process-per-test isolation keeps the
//! `OnceLock` cell and the cwd private to each scenario.

use base64::prelude::*;
use chrono::{Duration, Utc};
use serde_json::json;
use systemprompt_cloud::{CloudCredentials, CredentialsBootstrap, CredentialsBootstrapError};
use systemprompt_identifiers::{CloudAuthToken, Email};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn jwt_with_exp(offset_secs: i64) -> String {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let exp = Utc::now().timestamp() + offset_secs;
    let payload = BASE64_URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{exp}}}"#));
    let signature = BASE64_URL_SAFE_NO_PAD.encode("sig");
    format!("{header}.{payload}.{signature}")
}

fn project_root() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().expect("tempdir");
    let root = temp.path().canonicalize().expect("canonical root");
    std::fs::create_dir_all(root.join(".systemprompt")).expect("mk .systemprompt");
    std::fs::write(root.join("Cargo.toml"), "[package]\n").expect("write Cargo.toml");
    unsafe {
        std::env::remove_var("FLY_APP_NAME");
    }
    std::env::set_current_dir(&root).expect("chdir");
    (temp, root)
}

fn write_credentials(
    root: &std::path::Path,
    api_url: &str,
    token_offset_secs: i64,
    last_validated_at: Option<chrono::DateTime<Utc>>,
) -> std::path::PathBuf {
    let creds = CloudCredentials {
        api_token: CloudAuthToken::new(jwt_with_exp(token_offset_secs)),
        api_url: api_url.to_owned(),
        authenticated_at: Utc::now(),
        user_email: Email::new("dev@example.com".to_owned()),
        last_validated_at,
    };
    let path = root.join(".systemprompt/credentials.json");
    creds.save_to_path(&path).expect("save credentials");
    path
}

async fn mount_auth_me_ok(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": { "id": "user-1", "email": "dev@example.com" },
            "tenants": []
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn init_within_validation_ttl_skips_api_round_trip() {
    let (_guard, root) = project_root();
    write_credentials(&root, "http://127.0.0.1:9", 7200, Some(Utc::now()));

    let creds = CredentialsBootstrap::init()
        .await
        .expect("init")
        .expect("credentials present");
    assert_eq!(creds.user_email.as_str(), "dev@example.com");
    assert!(CredentialsBootstrap::is_initialized());
}

#[tokio::test]
async fn init_validates_and_persists_validation_timestamp() {
    let (_guard, root) = project_root();
    let server = MockServer::start().await;
    mount_auth_me_ok(&server).await;
    let creds_path = write_credentials(&root, &server.uri(), 7200, None);

    CredentialsBootstrap::init()
        .await
        .expect("init")
        .expect("credentials present");

    let persisted = CloudCredentials::load_from_path(&creds_path).expect("reload file");
    assert!(
        persisted.last_validated_at.is_some(),
        "validation timestamp must be persisted"
    );
}

#[tokio::test]
async fn init_revalidates_when_token_expires_within_an_hour() {
    let (_guard, root) = project_root();
    let server = MockServer::start().await;
    mount_auth_me_ok(&server).await;
    write_credentials(&root, &server.uri(), 1800, Some(Utc::now()));

    let creds = CredentialsBootstrap::init()
        .await
        .expect("init")
        .expect("credentials present");
    assert!(creds.expires_within(Duration::hours(1)));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn init_rejects_expired_token() {
    let (_guard, root) = project_root();
    write_credentials(&root, "http://127.0.0.1:9", -3600, None);

    let err = CredentialsBootstrap::init().await.unwrap_err();
    assert!(err.to_string().contains("expired"), "got {err}");
    assert!(!CredentialsBootstrap::is_initialized());
}

#[tokio::test]
async fn init_reports_missing_credentials_file() {
    let (_guard, _root) = project_root();

    let err = CredentialsBootstrap::init().await.unwrap_err();
    let message = err.to_string();
    assert!(message.contains("not found"), "got {message}");
}

#[tokio::test]
async fn reload_revalidates_stored_credentials() {
    let (_guard, root) = project_root();
    let server = MockServer::start().await;
    mount_auth_me_ok(&server).await;
    write_credentials(&root, &server.uri(), 7200, Some(Utc::now()));

    let creds = CredentialsBootstrap::reload().await.expect("reload");
    assert_eq!(creds.user_email.as_str(), "dev@example.com");
    assert_eq!(creds.api_url, server.uri());
}

#[tokio::test]
async fn reload_maps_corrupt_file_to_invalid_credentials() {
    let (_guard, root) = project_root();
    std::fs::write(root.join(".systemprompt/credentials.json"), "not-json").expect("write");

    let err = CredentialsBootstrap::reload().await.unwrap_err();
    assert!(matches!(
        err,
        CredentialsBootstrapError::InvalidCredentials { .. }
    ));
}

#[tokio::test]
async fn reload_maps_api_rejection_to_validation_failed() {
    let (_guard, root) = project_root();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    write_credentials(&root, &server.uri(), 7200, Some(Utc::now()));

    let err = CredentialsBootstrap::reload().await.unwrap_err();
    assert!(matches!(
        err,
        CredentialsBootstrapError::ApiValidationFailed { .. }
    ));
}
