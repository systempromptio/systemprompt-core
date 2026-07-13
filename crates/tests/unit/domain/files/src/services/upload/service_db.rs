//! End-to-end tests for `FileUploadService::upload_file` and its
//! `FileUploadProvider` trait impl: persistence modes, size/decode rejection,
//! path-traversal rejection, and DB-failure cleanup. Each test runs in its
//! own nextest process and writes its own `files.yaml` before building the
//! service's `FilesConfig`.

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use systemprompt_database::{Database, DbPool};
use systemprompt_files::{
    FileRepository, FileUploadError, FileUploadRequest, FileUploadService, FilesConfig,
};
use systemprompt_identifiers::{ContextId, SessionId, TraceId, UserId};
use systemprompt_test_fixtures::{TestBootstrap, ensure_test_bootstrap, fixture_db_pool};
use systemprompt_traits::{FileUploadInput, FileUploadProvider, FileUploadProviderError};

const CONTENT: &[u8] = b"hello upload bytes";
const CONTENT_SHA256: &str = "3cd6a1084a7842942497e88607bde216d55fa542bb5d1dea4fda4aca73f7f4c3";

fn files_config(bootstrap: &TestBootstrap, yaml: Option<&str>) -> FilesConfig {
    if let Some(content) = yaml {
        std::fs::write(bootstrap.services_path.join("config/files.yaml"), content)
            .expect("write files.yaml");
    }
    FilesConfig::from_profile(&bootstrap.app_paths).expect("from_profile")
}

async fn live_pool(bootstrap: &TestBootstrap) -> Option<DbPool> {
    fixture_db_pool(&bootstrap.database_url).await.ok()
}

fn encoded_content() -> String {
    STANDARD.encode(CONTENT)
}

fn regular_files_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found
}

#[tokio::test]
async fn upload_context_scoped_persists_file_and_row() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    let cfg = files_config(b, None);
    let service = FileUploadService::new(&pool, cfg.clone()).expect("service");
    assert!(service.is_enabled());

    let context_id = ContextId::generate();
    let request = FileUploadRequest::builder("image/png", encoded_content(), context_id.clone())
        .with_session_id(SessionId::new("sess-upload-ctx"))
        .with_trace_id(TraceId::new("trace-upload-ctx"))
        .build();

    let uploaded = service.upload_file(request).await.expect("upload");

    let expected_rel = format!(
        "contexts/{}/images/{}.png",
        context_id.as_str(),
        uploaded.file_id.as_str()
    );
    assert_eq!(uploaded.path, expected_rel);
    assert_eq!(uploaded.public_url, cfg.upload_url(&expected_rel));
    assert_eq!(uploaded.size_bytes, CONTENT.len() as i64);

    let on_disk = cfg.uploads().join(&expected_rel);
    assert_eq!(std::fs::read(&on_disk).expect("stored file"), CONTENT);

    let repo = FileRepository::new(&pool).expect("repo");
    let row = repo
        .find_by_id(&uploaded.file_id)
        .await
        .expect("find")
        .expect("row present");
    assert_eq!(row.mime_type, "image/png");
    assert_eq!(row.size_bytes, Some(CONTENT.len() as i64));
    assert_eq!(row.public_url, uploaded.public_url);
    assert_eq!(
        row.context_id.as_ref().map(ContextId::as_str),
        Some(context_id.as_str())
    );
    assert_eq!(
        row.session_id.as_ref().map(SessionId::as_str),
        Some("sess-upload-ctx")
    );
    assert_eq!(
        row.trace_id.as_ref().map(TraceId::as_str),
        Some("trace-upload-ctx")
    );
    let checksums = row.metadata.0.checksums.as_ref().expect("checksums");
    assert_eq!(checksums.sha256.as_deref(), Some(CONTENT_SHA256));

    repo.delete(&uploaded.file_id).await.expect("cleanup");
}

#[tokio::test]
async fn upload_rejected_when_persistence_disabled() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    let cfg = files_config(
        b,
        Some("files:\n  upload:\n    persistence_mode: disabled\n"),
    );
    let service = FileUploadService::new(&pool, cfg).expect("service");

    assert!(!FileUploadService::is_enabled(&service));
    assert!(!FileUploadProvider::is_enabled(&service));

    let request =
        FileUploadRequest::builder("image/png", encoded_content(), ContextId::generate()).build();
    let err = service.upload_file(request).await.expect_err("disabled");
    assert!(matches!(err, FileUploadError::PersistenceDisabled));

    let input = FileUploadInput::new("image/png", encoded_content(), None);
    let provider_err = FileUploadProvider::upload_file(&service, input)
        .await
        .expect_err("disabled via provider");
    match provider_err {
        FileUploadProviderError::StorageError { message } => {
            assert_eq!(message, "File persistence is disabled");
        },
        other => panic!("expected StorageError, got {other:?}"),
    }
}

#[tokio::test]
async fn upload_rejects_oversized_base64_payload() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    let cfg = files_config(b, Some("files:\n  upload:\n    max_file_size_bytes: 16\n"));
    let service = FileUploadService::new(&pool, cfg).expect("service");

    let oversized = STANDARD.encode(vec![7_u8; 4096]);
    let expected_len = oversized.len();
    let request = FileUploadRequest::builder("image/png", oversized, ContextId::generate()).build();

    let err = service.upload_file(request).await.expect_err("too large");
    match err {
        FileUploadError::Base64TooLarge { encoded_size } => {
            assert_eq!(encoded_size, expected_len);
        },
        other => panic!("expected Base64TooLarge, got {other:?}"),
    }
}

#[tokio::test]
async fn upload_rejects_invalid_base64() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    let service = FileUploadService::new(&pool, files_config(b, None)).expect("service");

    let request =
        FileUploadRequest::builder("image/png", "@@not-base64@@", ContextId::generate()).build();
    let err = service.upload_file(request).await.expect_err("bad base64");
    assert!(matches!(err, FileUploadError::Base64Decode(_)));
}

#[tokio::test]
async fn upload_user_library_scopes_path_to_user() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    let cfg = files_config(
        b,
        Some("files:\n  upload:\n    persistence_mode: user_library\n"),
    );
    let service = FileUploadService::new(&pool, cfg).expect("service");

    let user = UserId::new("upload-lib-user");
    let request = FileUploadRequest::builder("image/png", encoded_content(), ContextId::generate())
        .with_user_id(user.clone())
        .build();

    let uploaded = service.upload_file(request).await.expect("upload");
    assert_eq!(
        uploaded.path,
        format!(
            "users/{}/images/{}.png",
            user.as_str(),
            uploaded.file_id.as_str()
        )
    );

    let repo = FileRepository::new(&pool).expect("repo");
    repo.delete(&uploaded.file_id).await.expect("cleanup");
}

#[tokio::test]
async fn upload_user_library_without_user_uses_anonymous() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    let cfg = files_config(
        b,
        Some("files:\n  upload:\n    persistence_mode: user_library\n"),
    );
    let service = FileUploadService::new(&pool, cfg).expect("service");

    let request =
        FileUploadRequest::builder("image/png", encoded_content(), ContextId::generate()).build();

    let uploaded = service.upload_file(request).await.expect("upload");
    assert_eq!(
        uploaded.path,
        format!("users/anonymous/images/{}.png", uploaded.file_id.as_str())
    );

    let repo = FileRepository::new(&pool).expect("repo");
    repo.delete(&uploaded.file_id).await.expect("cleanup");
}

#[tokio::test]
async fn upload_rejects_user_id_with_traversal() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    let cfg = files_config(
        b,
        Some("files:\n  upload:\n    persistence_mode: user_library\n"),
    );
    let service = FileUploadService::new(&pool, cfg).expect("service");

    let request = FileUploadRequest::builder("image/png", encoded_content(), ContextId::generate())
        .with_user_id(UserId::new("../root"))
        .build();

    let err = service.upload_file(request).await.expect_err("traversal");
    match err {
        FileUploadError::PathValidation(message) => {
            assert_eq!(message, "Invalid user_id: contains path traversal sequence");
        },
        other => panic!("expected PathValidation, got {other:?}"),
    }
}

#[tokio::test]
async fn upload_db_failure_removes_stored_file() {
    let b = ensure_test_bootstrap();
    if live_pool(b).await.is_none() {
        return;
    }
    let cfg = files_config(b, None);

    // Read pool works so construction succeeds; the closed write pool makes
    // the insert fail after the file has been written, driving cleanup.
    let read = sqlx::PgPool::connect(&b.database_url)
        .await
        .expect("read pool");
    let closed = sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed")
        .expect("lazy pool");
    closed.close().await;
    let pool: DbPool = Arc::new(Database::from_pools(Arc::new(read), Some(Arc::new(closed))));

    let service = FileUploadService::new(&pool, cfg.clone()).expect("service");
    let context_id = ContextId::generate();
    let request =
        FileUploadRequest::builder("image/png", encoded_content(), context_id.clone()).build();

    let err = service.upload_file(request).await.expect_err("db failure");
    assert!(matches!(err, FileUploadError::Database(_)));

    let context_dir = cfg
        .uploads()
        .join(format!("contexts/{}", context_id.as_str()));
    assert!(
        regular_files_under(&context_dir).is_empty(),
        "stored artefact must be cleaned up after DB failure"
    );
}

#[tokio::test]
async fn provider_upload_generates_context_and_round_trips() {
    let b = ensure_test_bootstrap();
    let Some(pool) = live_pool(b).await else {
        return;
    };
    let service = FileUploadService::new(&pool, files_config(b, None)).expect("service");

    let mut input = FileUploadInput::new("image/png", encoded_content(), None);
    input.name = Some("holiday.png".to_owned());
    input.user_id = Some(UserId::new("provider-upload-user"));
    input.session_id = Some(SessionId::new("sess-provider-upload"));
    input.trace_id = Some(TraceId::new("trace-provider-upload"));

    let info = FileUploadProvider::upload_file(&service, input)
        .await
        .expect("provider upload");
    assert_eq!(info.size_bytes, Some(CONTENT.len() as i64));

    let repo = FileRepository::new(&pool).expect("repo");
    let row = repo
        .find_by_id(&info.file_id)
        .await
        .expect("find")
        .expect("row present");
    assert_eq!(row.public_url, info.public_url);
    assert!(
        row.context_id.is_some(),
        "provider generates a context id when none is supplied"
    );
    assert_eq!(
        row.user_id.as_ref().map(UserId::as_str),
        Some("provider-upload-user")
    );
    assert_eq!(
        row.session_id.as_ref().map(SessionId::as_str),
        Some("sess-provider-upload")
    );
    assert_eq!(
        row.trace_id.as_ref().map(TraceId::as_str),
        Some("trace-provider-upload")
    );

    repo.delete(&info.file_id).await.expect("cleanup");
}
