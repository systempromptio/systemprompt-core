//! DB-backed tests for [`FilesAiPersistenceProvider`], the bridge between the
//! AI image pipeline's [`AiFilePersistenceProvider`] trait and the files
//! [`FileRepository`].
//!
//! Each test uses unique ids and removes the rows it inserts. `storage_config`
//! is not exercised here: it reads the process-global `FilesConfig`, which is
//! only initialised through the full AppContext bootstrap (covered by the
//! integration suite).

use systemprompt_database::DbPool;
use systemprompt_files::{FileRepository, FilesAiPersistenceProvider};
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use systemprompt_traits::{AiFilePersistenceProvider, InsertAiFileParams};

async fn db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn new_uuid() -> uuid::Uuid {
    uuid::Uuid::new_v4()
}

fn insert_params(id: uuid::Uuid, user: &UserId) -> InsertAiFileParams {
    InsertAiFileParams {
        id,
        path: format!("/storage/generated/{id}.png"),
        public_url: format!("/files/images/generated/{id}.png"),
        mime_type: "image/png".to_owned(),
        size_bytes: Some(2048),
        metadata: serde_json::json!({ "prompt": "a cat" }),
        user_id: Some(user.clone()),
        session_id: Some(SessionId::new(format!("sess-{}", id.simple()))),
        trace_id: Some(TraceId::new(format!("trace-{}", id.simple()))),
        context_id: Some(ContextId::generate()),
    }
}

async fn cleanup(provider: &FilesAiPersistenceProvider, id: &FileId) {
    provider.delete(id).await.ok();
}

#[tokio::test]
async fn new_constructs_from_pool() {
    let Some(db) = db().await else { return };
    drop(FilesAiPersistenceProvider::new(&db).expect("new"));
}

#[tokio::test]
async fn from_repository_constructs() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let _ = FilesAiPersistenceProvider::from_repository(repo);
}

#[tokio::test]
async fn insert_then_find_by_id_round_trips_all_fields() {
    let Some(db) = db().await else { return };
    let provider = FilesAiPersistenceProvider::new(&db).expect("provider");
    let id = new_uuid();
    let file_id = FileId::new(id.to_string());
    let user = UserId::new(format!("u-{}", id.simple()));
    let params = insert_params(id, &user);

    provider.insert_file(params).await.expect("insert");

    let found = provider
        .find_by_id(&file_id)
        .await
        .expect("find")
        .expect("present");

    assert_eq!(found.id, id);
    assert_eq!(found.mime_type, "image/png");
    assert_eq!(found.size_bytes, Some(2048));
    assert!(found.ai_content, "ai_content flag persisted as true");
    assert_eq!(found.metadata["prompt"], "a cat");
    assert_eq!(
        found.user_id.as_ref().map(UserId::as_str),
        Some(user.as_str())
    );
    assert!(found.session_id.is_some());
    assert!(found.trace_id.is_some());
    assert!(found.context_id.is_some());

    cleanup(&provider, &file_id).await;
}

#[tokio::test]
async fn insert_without_optional_fields_persists() {
    let Some(db) = db().await else { return };
    let provider = FilesAiPersistenceProvider::new(&db).expect("provider");
    let id = new_uuid();
    let file_id = FileId::new(id.to_string());
    let params = InsertAiFileParams {
        id,
        path: format!("/storage/generated/{id}.png"),
        public_url: format!("/files/images/generated/{id}.png"),
        mime_type: "image/png".to_owned(),
        size_bytes: None,
        metadata: serde_json::json!({}),
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
    };

    provider.insert_file(params).await.expect("insert");
    let found = provider
        .find_by_id(&file_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.size_bytes, None);
    assert!(found.user_id.is_none());
    assert!(found.session_id.is_none());

    cleanup(&provider, &file_id).await;
}

#[tokio::test]
async fn find_by_id_missing_returns_none() {
    let Some(db) = db().await else { return };
    let provider = FilesAiPersistenceProvider::new(&db).expect("provider");
    let missing = FileId::new(uuid::Uuid::new_v4().to_string());
    let r = provider.find_by_id(&missing).await.expect("find");
    assert!(r.is_none());
}

#[tokio::test]
async fn list_by_user_returns_inserted_files() {
    let Some(db) = db().await else { return };
    let provider = FilesAiPersistenceProvider::new(&db).expect("provider");
    let user = UserId::new(format!("u-{}", uuid::Uuid::new_v4().simple()));
    let id_a = new_uuid();
    let id_b = new_uuid();
    let fid_a = FileId::new(id_a.to_string());
    let fid_b = FileId::new(id_b.to_string());

    provider
        .insert_file(insert_params(id_a, &user))
        .await
        .expect("insert a");
    provider
        .insert_file(insert_params(id_b, &user))
        .await
        .expect("insert b");

    let files = provider
        .list_by_user(&user, 50, 0)
        .await
        .expect("list by user");
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|f| f.ai_content));
    assert!(files.iter().any(|f| f.id == id_a));
    assert!(files.iter().any(|f| f.id == id_b));

    cleanup(&provider, &fid_a).await;
    cleanup(&provider, &fid_b).await;
}

#[tokio::test]
async fn list_by_user_respects_limit() {
    let Some(db) = db().await else { return };
    let provider = FilesAiPersistenceProvider::new(&db).expect("provider");
    let user = UserId::new(format!("u-{}", uuid::Uuid::new_v4().simple()));
    let id_a = new_uuid();
    let id_b = new_uuid();
    let fid_a = FileId::new(id_a.to_string());
    let fid_b = FileId::new(id_b.to_string());
    provider
        .insert_file(insert_params(id_a, &user))
        .await
        .expect("insert a");
    provider
        .insert_file(insert_params(id_b, &user))
        .await
        .expect("insert b");

    let limited = provider.list_by_user(&user, 1, 0).await.expect("list");
    assert_eq!(limited.len(), 1, "limit of 1 returns a single row");

    cleanup(&provider, &fid_a).await;
    cleanup(&provider, &fid_b).await;
}

#[tokio::test]
async fn list_by_user_empty_for_unknown_user() {
    let Some(db) = db().await else { return };
    let provider = FilesAiPersistenceProvider::new(&db).expect("provider");
    let user = UserId::new(format!("u-{}", uuid::Uuid::new_v4().simple()));
    let files = provider.list_by_user(&user, 10, 0).await.expect("list");
    assert!(files.is_empty());
}

#[tokio::test]
async fn delete_soft_deletes_file() {
    let Some(db) = db().await else { return };
    let provider = FilesAiPersistenceProvider::new(&db).expect("provider");
    let id = new_uuid();
    let file_id = FileId::new(id.to_string());
    let user = UserId::new(format!("u-{}", id.simple()));
    provider
        .insert_file(insert_params(id, &user))
        .await
        .expect("insert");

    provider.delete(&file_id).await.expect("delete");

    let after = provider.find_by_id(&file_id).await.expect("find");
    assert!(after.is_none(), "deleted file no longer found");
}
