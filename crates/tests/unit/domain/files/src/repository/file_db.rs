//! DB-backed `FileRepository` branches: invalid-UUID rejection, `insert_file`
//! propagation of every optional field, and the global AI-image counters.

use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_files::{File, FileMetadata, FileRepository, FilesError, InsertFileRequest};
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn ai_file(id: uuid::Uuid, user: &UserId) -> File {
    let now = Utc::now();
    File {
        id,
        path: format!("/storage/ai-count/{id}.png"),
        public_url: format!("/files/ai-count/{id}.png"),
        mime_type: "image/png".to_owned(),
        size_bytes: Some(9),
        ai_content: true,
        metadata: sqlx::types::Json(FileMetadata::default()),
        user_id: Some(user.clone()),
        session_id: Some(SessionId::new(format!("sess-{}", id.simple()))),
        trace_id: Some(TraceId::new(format!("trace-{}", id.simple()))),
        context_id: Some(ContextId::generate()),
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }
}

#[tokio::test]
async fn insert_rejects_non_uuid_file_id() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");

    let request = InsertFileRequest::new(
        FileId::new("not-a-uuid"),
        "/storage/bad-id.png",
        "/files/bad-id.png",
        "image/png",
    );

    let err = repo.insert(request).await.expect_err("invalid uuid");
    match err {
        FilesError::Validation(message) => {
            assert!(
                message.contains("Invalid UUID for file id not-a-uuid"),
                "unexpected message: {message}"
            );
        },
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[tokio::test]
async fn insert_file_round_trips_all_optional_fields() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let id = uuid::Uuid::new_v4();
    let user = UserId::new(format!("u-{}", id.simple()));
    let file = ai_file(id, &user);
    let context_id = file.context_id.clone().expect("context set");

    let file_id = repo.insert_file(&file).await.expect("insert_file");
    assert_eq!(file_id.as_str(), id.to_string());

    let row = repo
        .find_by_id(&file_id)
        .await
        .expect("find")
        .expect("row present");
    assert_eq!(row.size_bytes, Some(9));
    assert!(row.ai_content);
    assert_eq!(
        row.user_id.as_ref().map(UserId::as_str),
        Some(user.as_str())
    );
    assert_eq!(
        row.session_id.as_ref().map(SessionId::as_str),
        Some(format!("sess-{}", id.simple()).as_str())
    );
    assert_eq!(
        row.trace_id.as_ref().map(TraceId::as_str),
        Some(format!("trace-{}", id.simple()).as_str())
    );
    assert_eq!(
        row.context_id.as_ref().map(ContextId::as_str),
        Some(context_id.as_str())
    );

    repo.delete(&file_id).await.expect("cleanup");
}

#[tokio::test]
async fn count_ai_images_reflects_inserted_rows() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let user = UserId::new(format!("u-{}", uuid::Uuid::new_v4().simple()));

    assert_eq!(
        repo.count_ai_images_by_user(&user).await.expect("per-user"),
        0
    );

    let id_a = uuid::Uuid::new_v4();
    let id_b = uuid::Uuid::new_v4();
    repo.insert_file(&ai_file(id_a, &user))
        .await
        .expect("insert a");
    repo.insert_file(&ai_file(id_b, &user))
        .await
        .expect("insert b");

    // Sibling test processes insert and delete AI rows concurrently, so the
    // global counter only supports a lower bound; the per-user count is exact.
    let after_global = repo.count_ai_images().await.expect("count after");
    assert!(after_global >= 2, "global count was {after_global}");
    assert_eq!(
        repo.count_ai_images_by_user(&user).await.expect("per-user"),
        2
    );

    repo.delete(&FileId::new(id_a.to_string()))
        .await
        .expect("cleanup a");
    repo.delete(&FileId::new(id_b.to_string()))
        .await
        .expect("cleanup b");
}
