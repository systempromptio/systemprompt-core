//! Error and configuration arms of `FilesAiPersistenceProvider`: invalid
//! metadata rejection, closed-pool error mapping on every trait method, and
//! `storage_config` with and without the global `FilesConfig`.

use systemprompt_files::FilesAiPersistenceProvider;
use systemprompt_identifiers::{FileId, UserId};
use systemprompt_test_fixtures::{closed_db_pool, ensure_test_bootstrap};
use systemprompt_traits::{AiFilePersistenceProvider, AiProviderError, InsertAiFileParams};

fn params(metadata: serde_json::Value) -> InsertAiFileParams {
    let id = uuid::Uuid::new_v4();
    InsertAiFileParams {
        id,
        path: format!("/storage/generated/{id}.png"),
        public_url: format!("/files/images/generated/{id}.png"),
        mime_type: "image/png".to_owned(),
        size_bytes: Some(64),
        metadata,
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
    }
}

#[tokio::test]
async fn insert_file_rejects_non_object_metadata() {
    let provider = FilesAiPersistenceProvider::new(&closed_db_pool().await).expect("provider");

    let err = provider
        .insert_file(params(serde_json::json!("not-an-object")))
        .await
        .expect_err("invalid metadata");
    match err {
        AiProviderError::Internal(message) => {
            assert!(
                message.contains("Invalid file metadata"),
                "unexpected message: {message}"
            );
        },
        other => panic!("expected Internal, got {other:?}"),
    }
}

#[tokio::test]
async fn closed_pool_maps_every_method_to_internal() {
    let provider = FilesAiPersistenceProvider::new(&closed_db_pool().await).expect("provider");
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let user = UserId::new("ai-closed-pool-user");

    let insert_err = provider
        .insert_file(params(serde_json::json!({})))
        .await
        .expect_err("insert on closed pool");
    assert!(matches!(insert_err, AiProviderError::Internal(_)));

    let find_err = provider
        .find_by_id(&file_id)
        .await
        .expect_err("find on closed pool");
    assert!(matches!(find_err, AiProviderError::Internal(_)));

    let list_err = provider
        .list_by_user(&user, 10, 0)
        .await
        .expect_err("list on closed pool");
    assert!(matches!(list_err, AiProviderError::Internal(_)));

    let delete_err = provider
        .delete(&file_id)
        .await
        .expect_err("delete on closed pool");
    assert!(matches!(delete_err, AiProviderError::Internal(_)));
}

#[tokio::test]
async fn storage_config_reflects_initialised_files_config() {
    let b = ensure_test_bootstrap();
    let provider = FilesAiPersistenceProvider::new(&closed_db_pool().await).expect("provider");

    let config = provider.storage_config().expect("storage config");
    assert_eq!(
        config.base_path,
        b.storage_path.join("files/images/generated")
    );
    assert_eq!(config.url_prefix, "/files/images/generated");
}

#[tokio::test]
async fn storage_config_without_global_config_is_configuration_error() {
    // No bootstrap: the process-global FilesConfig is uninitialised.
    let provider = FilesAiPersistenceProvider::new(&closed_db_pool().await).expect("provider");

    let err = provider.storage_config().expect_err("config missing");
    match err {
        AiProviderError::ConfigurationError { message } => {
            assert!(
                message.contains("FilesConfig::init() not called"),
                "unexpected message: {message}"
            );
        },
        other => panic!("expected ConfigurationError, got {other:?}"),
    }
}
