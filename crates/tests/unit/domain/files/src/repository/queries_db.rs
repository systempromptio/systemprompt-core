//! DB-backed `FileRepository` query methods: AI-image listing, `list_all`,
//! `update_metadata`, and `search_by_path`, each on uniquely-tagged rows.

use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_files::{File, FileChecksums, FileMetadata, FileRepository};
use systemprompt_identifiers::{FileId, UserId};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn file_row(tag: &str, ai_content: bool, user: Option<&UserId>) -> File {
    let id = uuid::Uuid::new_v4();
    let now = Utc::now();
    File {
        id,
        path: format!("/storage/{tag}/{id}.png"),
        public_url: format!("/files/{tag}/{id}.png"),
        mime_type: "image/png".to_owned(),
        size_bytes: Some(5),
        ai_content,
        metadata: sqlx::types::Json(FileMetadata::default()),
        user_id: user.cloned(),
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }
}

#[tokio::test]
async fn list_ai_images_includes_inserted_ai_rows() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let tag = format!("ai-list-{}", uuid::Uuid::new_v4().simple());
    let user = UserId::new(format!("u-{tag}"));

    let ai = file_row(&tag, true, Some(&user));
    let plain = file_row(&tag, false, Some(&user));
    repo.insert_file(&ai).await.expect("insert ai");
    repo.insert_file(&plain).await.expect("insert plain");

    let global = repo.list_ai_images(1000, 0).await.expect("list global");
    assert!(global.iter().any(|f| f.id == ai.id));
    assert!(
        !global.iter().any(|f| f.id == plain.id),
        "non-AI rows excluded from AI listing"
    );

    let by_user = repo
        .list_ai_images_by_user(&user, 10, 0)
        .await
        .expect("list by user");
    assert_eq!(by_user.len(), 1);
    assert_eq!(by_user[0].id, ai.id);
    assert_eq!(by_user[0].path, ai.path);

    repo.delete(&FileId::new(ai.id.to_string()))
        .await
        .expect("cleanup ai");
    repo.delete(&FileId::new(plain.id.to_string()))
        .await
        .expect("cleanup plain");
}

#[tokio::test]
async fn list_all_and_search_by_path_return_tagged_rows() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let tag = format!("query-{}", uuid::Uuid::new_v4().simple());

    let file = file_row(&tag, false, None);
    repo.insert_file(&file).await.expect("insert");

    let all = repo.list_all(1000, 0).await.expect("list_all");
    assert!(all.iter().any(|f| f.id == file.id));

    let matches = repo.search_by_path(&tag, 10).await.expect("search");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, file.id);
    assert_eq!(matches[0].path, file.path);

    let no_matches = repo
        .search_by_path(&format!("{tag}-missing"), 10)
        .await
        .expect("search miss");
    assert!(no_matches.is_empty());

    repo.delete(&FileId::new(file.id.to_string()))
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn update_metadata_persists_new_checksums() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let tag = format!("meta-{}", uuid::Uuid::new_v4().simple());

    let file = file_row(&tag, false, None);
    repo.insert_file(&file).await.expect("insert");
    let file_id = FileId::new(file.id.to_string());

    let metadata =
        FileMetadata::new().with_checksums(FileChecksums::new().with_sha256("abc123def456"));
    repo.update_metadata(&file_id, &metadata)
        .await
        .expect("update metadata");

    let row = repo
        .find_by_id(&file_id)
        .await
        .expect("find")
        .expect("row present");
    assert_eq!(
        row.metadata
            .0
            .checksums
            .as_ref()
            .and_then(|c| c.sha256.as_deref()),
        Some("abc123def456")
    );

    repo.delete(&file_id).await.expect("cleanup");
}

#[tokio::test]
async fn get_stats_snapshot_is_internally_consistent() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let tag = format!("stats-{}", uuid::Uuid::new_v4().simple());

    let file = file_row(&tag, true, None);
    repo.insert_file(&file).await.expect("insert");

    let after = repo.get_stats().await.expect("stats after");
    // Sibling test processes insert and delete rows concurrently, so only
    // lower bounds and the per-snapshot categorical identity are stable.
    assert!(after.total_files >= 1);
    assert!(after.image_count >= 1);
    assert!(after.ai_images_count >= 1);
    assert!(after.total_size_bytes >= 5);
    assert_eq!(
        after.other_count,
        (after.total_files
            - after.image_count
            - after.document_count
            - after.audio_count
            - after.video_count)
            .max(0)
    );
    assert_eq!(
        after.other_size_bytes,
        (after.total_size_bytes
            - after.image_size_bytes
            - after.document_size_bytes
            - after.audio_size_bytes
            - after.video_size_bytes)
            .max(0)
    );

    repo.delete(&FileId::new(file.id.to_string()))
        .await
        .expect("cleanup");
}
