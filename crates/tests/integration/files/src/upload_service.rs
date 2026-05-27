//! Integration tests for FileUploadService.
//!
//! Requires `DATABASE_URL` to point at a migrated Postgres instance. Bootstraps
//! `ProfileBootstrap` + `FilesConfig` via the shared `bootstrap` helper so the
//! upload service can resolve storage roots.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use systemprompt_database::DbPool;
use systemprompt_files::{
    FileUploadError, FileUploadRequest, FileUploadService, FileValidator, FilesConfig,
};
use systemprompt_identifiers::{ContextId, SessionId, TraceId, UserId};

use crate::bootstrap::test_env;

async fn get_db() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

fn one_pixel_png_base64() -> String {
    let bytes: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0x99, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x5B, 0xEF, 0x6A, 0xC8, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    STANDARD.encode(bytes)
}

fn unique_context_id(_suffix: &str) -> ContextId {
    ContextId::generate()
}

#[tokio::test]
async fn upload_service_new_and_is_enabled() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping (no db)");
        return;
    };
    let _env = test_env();
    let files_config = FilesConfig::get().expect("FilesConfig::get").clone();

    let service =
        FileUploadService::new(&db, files_config).expect("FileUploadService::new should succeed");
    assert!(service.is_enabled(), "uploads should be enabled by default");
    let validator_ref = service.validator();
    let _ = format!("{validator_ref:?}");

    let ext = FileValidator::get_extension("image/png", None);
    assert_eq!(ext, "png");
    let ext_from_name = FileValidator::get_extension("application/octet-stream", Some("doc.pdf"));
    assert_eq!(ext_from_name, "pdf");
}

#[tokio::test]
async fn upload_service_uploads_png_successfully() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping (no db)");
        return;
    };
    let _env = test_env();
    let files_config = FilesConfig::get().expect("FilesConfig::get").clone();
    let service = FileUploadService::new(&db, files_config).expect("service");

    let request = FileUploadRequest::builder(
        "image/png",
        one_pixel_png_base64(),
        unique_context_id("png"),
    )
    .with_name("test.png")
    .with_user_id(UserId::new(format!("user_{}", uuid::Uuid::new_v4())))
    .with_session_id(SessionId::generate())
    .with_trace_id(TraceId::new(uuid::Uuid::new_v4().to_string()))
    .build();

    let uploaded = service.upload_file(request).await.expect("upload succeeds");
    assert!(!uploaded.path.is_empty(), "relative path populated");
    assert!(uploaded.public_url.contains("files/uploads/"));
    assert!(uploaded.size_bytes > 0);
}

#[tokio::test]
async fn upload_service_rejects_blocked_mime_type() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping (no db)");
        return;
    };
    let _env = test_env();
    let files_config = FilesConfig::get().expect("FilesConfig::get").clone();
    let service = FileUploadService::new(&db, files_config).expect("service");

    let request = FileUploadRequest::builder(
        "application/x-msdownload",
        one_pixel_png_base64(),
        unique_context_id("blocked"),
    )
    .build();

    let err = service.upload_file(request).await.expect_err("must reject");
    assert!(matches!(err, FileUploadError::Validation(_)));
}

#[tokio::test]
async fn upload_service_rejects_oversized_base64() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping (no db)");
        return;
    };
    let _env = test_env();
    let files_config = FilesConfig::get().expect("FilesConfig::get").clone();
    let service = FileUploadService::new(&db, files_config).expect("service");

    let huge = "A".repeat(120 * 1024 * 1024);
    let request = FileUploadRequest::builder("image/png", huge, unique_context_id("over")).build();

    let err = service.upload_file(request).await.expect_err("oversized");
    assert!(matches!(err, FileUploadError::Base64TooLarge { .. }));
}

#[tokio::test]
async fn upload_service_rejects_unknown_mime_type() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping (no db)");
        return;
    };
    let _env = test_env();
    let files_config = FilesConfig::get().expect("FilesConfig::get").clone();
    let service = FileUploadService::new(&db, files_config).expect("service");

    let request = FileUploadRequest::builder(
        "application/x-totally-fake",
        one_pixel_png_base64(),
        unique_context_id("unk"),
    )
    .build();

    let err = service.upload_file(request).await.expect_err("must reject");
    assert!(matches!(err, FileUploadError::Validation(_)));
}
