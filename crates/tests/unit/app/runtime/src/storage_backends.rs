//! The profile `storage:` section resolved through `AppContextBuilder::build`.
//! Storage is composed before the database is touched, so a misconfigured
//! object-store backend fails the boot with its own `RuntimeError` variant; a
//! well-formed one gets past storage and fails later on the missing admin row.
//! Only the failures profile validation cannot see (secret contents, URL
//! parsing) are reachable here.

use systemprompt_identifiers::SecretName;
use systemprompt_runtime::storage::ServiceAccountTokens;
use systemprompt_runtime::{AppContext, RuntimeError};
use systemprompt_security::google::ServiceAccountKey;
use systemprompt_storage::{GcsError, GcsTokenSource};

use crate::boot::{BootOptions, boot};

fn unique_admin(tag: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .subsec_nanos();
    format!("covstg_{tag}_{}_{nanos}", std::process::id())
}

async fn build_with(storage_yaml: &str, extra_secrets: Vec<(String, String)>) -> RuntimeError {
    let _fixture = boot(&BootOptions {
        admin_username: unique_admin("stg"),
        storage_yaml: storage_yaml.to_owned(),
        extra_secrets,
        ..BootOptions::default()
    });
    AppContext::builder()
        .with_extensions(systemprompt_extension::ExtensionRegistry::new())
        .build()
        .await
        .expect_err("no admin row is seeded, so the build must fail")
}

fn service_account_json() -> String {
    serde_json::json!({
        "client_email": "storage@fixture.iam.gserviceaccount.com",
        "private_key": "not a pem key",
        "project_id": "fixture-project",
    })
    .to_string()
}

#[tokio::test]
async fn gcs_naming_an_absent_secret_is_refused() {
    let err = build_with(
        "storage:\n  backend: gcs\n  bucket: bucket-a\n  credentials:\n    secret: gcs_key_absent\n",
        Vec::new(),
    )
    .await;
    match err {
        RuntimeError::StorageCredentialMissing { name } => {
            assert_eq!(name.as_str(), "gcs_key_absent");
        },
        other => panic!("expected StorageCredentialMissing, got {other:?}"),
    }
}

#[tokio::test]
async fn gcs_secret_that_is_not_a_service_account_key_is_refused() {
    let err = build_with(
        "storage:\n  backend: gcs\n  bucket: bucket-a\n  credentials:\n    secret: gcs_key_bad\n",
        vec![("gcs_key_bad".to_owned(), "{\"nope\": true}".to_owned())],
    )
    .await;
    match err {
        RuntimeError::StorageCredential { name, .. } => assert_eq!(name.as_str(), "gcs_key_bad"),
        other => panic!("expected StorageCredential, got {other:?}"),
    }
}

#[tokio::test]
async fn gcs_with_a_service_account_secret_composes_and_boot_moves_on() {
    let err = build_with(
        "storage:\n  backend: gcs\n  bucket: bucket-a\n  prefix: p\n  credentials:\n    secret: gcs_key_ok\n",
        vec![("gcs_key_ok".to_owned(), service_account_json())],
    )
    .await;
    assert!(
        matches!(err, RuntimeError::SystemAdminNotFound { .. }),
        "storage must compose; the boot must fail later on the admin row, got {err:?}"
    );
}

#[tokio::test]
async fn gcs_with_workload_identity_composes_and_boot_moves_on() {
    let err = build_with(
        "storage:\n  backend: gcs\n  bucket: bucket-a\n  public_read: true\n",
        Vec::new(),
    )
    .await;
    assert!(
        matches!(err, RuntimeError::SystemAdminNotFound { .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn s3_naming_an_absent_secret_is_refused() {
    let err = build_with(
        "storage:\n  backend: s3\n  bucket: bucket-a\n  credentials:\n    secret: s3_absent\n",
        Vec::new(),
    )
    .await;
    assert!(
        matches!(err, RuntimeError::StorageCredentialMissing { .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn s3_secret_without_access_keys_is_refused() {
    let err = build_with(
        "storage:\n  backend: s3\n  bucket: bucket-a\n  credentials:\n    secret: s3_bad\n",
        vec![("s3_bad".to_owned(), "[]".to_owned())],
    )
    .await;
    match err {
        RuntimeError::StorageCredential { name, .. } => assert_eq!(name.as_str(), "s3_bad"),
        other => panic!("expected StorageCredential, got {other:?}"),
    }
}

fn s3_secret(name: &str) -> Vec<(String, String)> {
    vec![(
        name.to_owned(),
        "{\"access_key_id\": \"AKIA\", \"secret_access_key\": \"shh\"}".to_owned(),
    )]
}

#[tokio::test]
async fn s3_with_an_unparseable_endpoint_is_refused() {
    let err = build_with(
        "storage:\n  backend: s3\n  bucket: bucket-a\n  endpoint: 'http://'\n  credentials:\n    secret: s3_ep\n",
        s3_secret("s3_ep"),
    )
    .await;
    assert!(
        matches!(err, RuntimeError::StorageEndpoint(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn s3_with_credentials_and_endpoint_composes_and_boot_moves_on() {
    let err = build_with(
        "storage:\n  backend: s3\n  bucket: bucket-a\n  prefix: p\n  region: eu-west-1\n  endpoint: http://127.0.0.1:9/\n  path_style: true\n  public_read: true\n  credentials:\n    secret: s3_ok\n",
        s3_secret("s3_ok"),
    )
    .await;
    assert!(
        matches!(err, RuntimeError::SystemAdminNotFound { .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn a_service_account_key_that_cannot_sign_yields_a_token_error() {
    let key: ServiceAccountKey =
        serde_json::from_str(&service_account_json()).expect("service account key json");
    let tokens = ServiceAccountTokens::new(&SecretName::new("gcs_unsignable"), key);
    let err = tokens
        .bearer()
        .await
        .expect_err("an unparseable private key cannot sign an assertion");
    assert!(matches!(err, GcsError::Token(_)), "got {err:?}");
}
