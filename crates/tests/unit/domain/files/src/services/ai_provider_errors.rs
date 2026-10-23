//! Error and configuration arms of `FilesAiPersistenceProvider`: invalid
//! metadata rejection, closed-pool error mapping on every trait method, and
//! `storage_config` derived from the injected `FilesConfig`.

use std::sync::Arc;

use systemprompt_files::{FilesAiPersistenceProvider, FilesConfig, FilesConfigYaml};
use systemprompt_identifiers::{FileId, UserId};
use systemprompt_test_fixtures::{closed_db_pool, ensure_test_bootstrap};
use systemprompt_traits::{AiFilePersistenceProvider, AiProviderError, InsertAiFileParams};

fn provider(repository: systemprompt_files::FileRepository) -> FilesAiPersistenceProvider {
    FilesAiPersistenceProvider::new(
        repository,
        Arc::new(FilesConfig::new(
            std::env::temp_dir(),
            FilesConfigYaml::default(),
        )),
    )
}

fn params(metadata: serde_json::Value) -> InsertAiFileParams {
    let id = uuid::Uuid::new_v4();
    InsertAiFileParams {
        id: FileId::new(id.to_string()),
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
    let provider = provider(systemprompt_files::FileRepository::new(
        &closed_db_pool().await,
    ));

    let err = provider
        .insert_file(params(serde_json::json!("not-an-object")))
        .await
        .expect_err("invalid metadata");
    match err {
        AiProviderError::Internal(cause) => {
            assert!(
                std::error::Error::source(cause.as_ref())
                    .is_some_and(|source| source.is::<serde_json::Error>()),
                "the metadata failure keeps its JSON cause: {cause}"
            );
        },
        other => panic!("expected Internal, got {other:?}"),
    }
}

#[tokio::test]
async fn closed_pool_maps_every_method_to_internal() {
    let provider = provider(systemprompt_files::FileRepository::new(
        &closed_db_pool().await,
    ));
    let file_id = FileId::generate();
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
async fn storage_config_reflects_the_injected_files_config() {
    let b = ensure_test_bootstrap();
    let provider = FilesAiPersistenceProvider::new(
        systemprompt_files::FileRepository::new(&closed_db_pool().await),
        Arc::new(b.files_config()),
    );

    let config = provider.storage_config().expect("storage config");
    assert_eq!(
        config.base_path,
        FilesConfig::generated_images_relative(),
        "generated images are addressed relative to paths.storage ({})",
        b.storage_path.display()
    );
    assert_eq!(config.url_prefix, "/files/images/generated");
}

#[tokio::test]
async fn storage_config_follows_a_custom_url_prefix() {
    let yaml: FilesConfigYaml =
        serde_yaml::from_str("urlPrefix: /assets").expect("files yaml parses");
    let provider = FilesAiPersistenceProvider::new(
        systemprompt_files::FileRepository::new(&closed_db_pool().await),
        Arc::new(FilesConfig::new(std::env::temp_dir(), yaml)),
    );

    let config = provider.storage_config().expect("storage config");
    assert_eq!(config.url_prefix, "/assets/images/generated");
}
