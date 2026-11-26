//! DB-backed tests for [`ContentFileRepository`], the content member's
//! `content_files` links, reading the stored files through the files
//! member's repository as the shared `LinkedFileSource`.
//!
//! Each test seeds its own `files` and `markdown_content` parents with unique
//! ids and removes them on completion so parallel runs do not collide.

use std::sync::Arc;

use systemprompt_content::{ContentError, ContentFileRepository};
use systemprompt_database::DbPool;
use systemprompt_extension::content_files::ContentFileRole;
use systemprompt_files::{FileRepository, InsertFileRequest};
use systemprompt_identifiers::{ContentId, ContextId, FileId};
use systemprompt_test_fixtures::test_db_pool;

fn new_content_id() -> ContentId {
    ContentId::new(format!("ct-test-{}", uuid::Uuid::new_v4().simple()))
}

fn links(db: &DbPool) -> ContentFileRepository {
    ContentFileRepository::new(db, Arc::new(FileRepository::new(db)))
}

async fn seed_file(db: &DbPool) -> FileId {
    let id = FileId::generate();
    let request = InsertFileRequest::new(
        id.clone(),
        format!("/storage/test/{}.png", id.as_str()),
        format!("/files/test/{}.png", id.as_str()),
        "image/png",
    )
    .with_size(128)
    .with_context_id(ContextId::generate());
    FileRepository::new(db)
        .insert(request)
        .await
        .expect("seed file");
    id
}

async fn seed_content(db: &DbPool) -> ContentId {
    let id = new_content_id();
    sqlx::query(
        r#"
        INSERT INTO markdown_content
            (id, slug, locale, title, description, body, author, published_at,
             keywords, kind, source_id, version_hash, public)
        VALUES ($1, $1, 'en', 'Title', 'Desc', 'Body', 'tester', CURRENT_TIMESTAMP,
                'kw', 'article', 'src-test', 'vh-test', true)
        "#,
    )
    .bind(id.as_str())
    .execute(db.pool().as_ref())
    .await
    .expect("seed markdown_content");
    id
}

async fn cleanup(db: &DbPool, content: &[&ContentId], files: &[&FileId]) {
    for id in content {
        sqlx::query("DELETE FROM markdown_content WHERE id = $1")
            .bind(id.as_str())
            .execute(db.pool().as_ref())
            .await
            .expect("cleanup content");
    }
    let repo = FileRepository::new(db);
    for id in files {
        repo.delete(id).await.expect("cleanup file");
    }
}

#[tokio::test]
async fn a_link_upserts_its_display_order_without_a_duplicate_row() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content = seed_content(&db).await;
    let file = seed_file(&db).await;

    repo.link(&content, &file, ContentFileRole::Attachment, 1)
        .await
        .expect("first link");
    repo.link(&content, &file, ContentFileRole::Attachment, 9)
        .await
        .expect("conflict upsert");

    let links_of = repo.contents_of(&file).await.expect("list");
    assert_eq!(links_of.len(), 1, "upsert must not create a duplicate row");
    assert_eq!(
        links_of[0].display_order, 9,
        "display_order updated on conflict"
    );
    assert_eq!(links_of[0].role, "attachment");

    cleanup(&db, &[&content], &[&file]).await;
}

#[tokio::test]
async fn a_non_uuid_file_id_is_refused_typed() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content = new_content_id();
    let bad = FileId::new("not-a-uuid");
    for err in [
        repo.link(&content, &bad, ContentFileRole::Attachment, 0)
            .await
            .expect_err("link"),
        repo.unlink(&content, &bad).await.expect_err("unlink"),
        repo.set_featured(&content, &bad)
            .await
            .expect_err("featured"),
        repo.contents_of(&bad).await.expect_err("contents"),
    ] {
        assert!(
            matches!(err, ContentError::InvalidFileId { .. }),
            "expected InvalidFileId, got {err:?}"
        );
    }
}

#[tokio::test]
async fn unlink_removes_the_association() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content = seed_content(&db).await;
    let file = seed_file(&db).await;
    repo.link(&content, &file, ContentFileRole::Attachment, 0)
        .await
        .expect("link");

    repo.unlink(&content, &file).await.expect("unlink");

    assert!(repo.contents_of(&file).await.expect("list").is_empty());
    cleanup(&db, &[&content], &[&file]).await;
}

#[tokio::test]
async fn files_of_returns_the_stored_files_in_link_order() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content = seed_content(&db).await;
    let first = seed_file(&db).await;
    let second = seed_file(&db).await;
    repo.link(&content, &second, ContentFileRole::Attachment, 5)
        .await
        .expect("link second");
    repo.link(&content, &first, ContentFileRole::Featured, 1)
        .await
        .expect("link first");

    let files = repo.files_of(&content).await.expect("files");

    assert_eq!(files.len(), 2);
    assert_eq!(
        files[0].file_id, first,
        "ordered by display_order ascending"
    );
    assert_eq!(files[0].display_order, 1);
    assert_eq!(files[0].role, "featured");
    assert_eq!(files[0].mime_type, "image/png");
    assert_eq!(files[1].file_id, second);
    assert_eq!(files[1].display_order, 5);
    cleanup(&db, &[&content], &[&first, &second]).await;
}

#[tokio::test]
async fn files_of_leaves_out_a_deleted_file() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content = seed_content(&db).await;
    let kept = seed_file(&db).await;
    let deleted = seed_file(&db).await;
    repo.link(&content, &kept, ContentFileRole::Attachment, 0)
        .await
        .expect("link kept");
    repo.link(&content, &deleted, ContentFileRole::Attachment, 1)
        .await
        .expect("link deleted");
    sqlx::query("UPDATE files SET deleted_at = CURRENT_TIMESTAMP WHERE id = $1::uuid")
        .bind(deleted.as_str())
        .execute(db.pool().as_ref())
        .await
        .expect("soft delete");

    let files = repo.files_of(&content).await.expect("files");

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_id, kept);
    cleanup(&db, &[&content], &[&kept, &deleted]).await;
}

#[tokio::test]
async fn files_of_is_empty_for_unknown_content() {
    let db = test_db_pool().await;
    let files = links(&db).files_of(&new_content_id()).await.expect("files");
    assert!(files.is_empty());
}

#[tokio::test]
async fn featured_returns_only_the_featured_file() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content = seed_content(&db).await;
    let attachment = seed_file(&db).await;
    repo.link(&content, &attachment, ContentFileRole::Attachment, 0)
        .await
        .expect("link attachment");
    assert!(repo.featured(&content).await.expect("featured").is_none());

    let featured = seed_file(&db).await;
    repo.link(&content, &featured, ContentFileRole::Featured, 0)
        .await
        .expect("link featured");

    let found = repo
        .featured(&content)
        .await
        .expect("featured")
        .expect("featured present");
    assert_eq!(found.file_id, featured);
    assert_eq!(found.role, "featured");
    cleanup(&db, &[&content], &[&attachment, &featured]).await;
}

#[tokio::test]
async fn set_featured_demotes_the_previous_featured_file() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content = seed_content(&db).await;
    let old_featured = seed_file(&db).await;
    let new_featured = seed_file(&db).await;
    repo.link(&content, &old_featured, ContentFileRole::Featured, 0)
        .await
        .expect("link old featured");
    repo.link(&content, &new_featured, ContentFileRole::Attachment, 1)
        .await
        .expect("link new as attachment");

    repo.set_featured(&content, &new_featured)
        .await
        .expect("set featured");

    let featured = repo
        .featured(&content)
        .await
        .expect("featured")
        .expect("present");
    assert_eq!(featured.file_id, new_featured, "new file is now featured");
    let old = repo.contents_of(&old_featured).await.expect("list old");
    assert_eq!(old[0].role, "attachment", "previous featured demoted");
    cleanup(&db, &[&content], &[&old_featured, &new_featured]).await;
}

#[tokio::test]
async fn set_featured_refuses_a_file_that_is_not_linked() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content = seed_content(&db).await;
    let unlinked = seed_file(&db).await;

    let err = repo
        .set_featured(&content, &unlinked)
        .await
        .expect_err("unlinked file cannot be featured");

    assert!(
        matches!(err, ContentError::FileNotLinked { .. }),
        "expected FileNotLinked, got {err:?}"
    );
    cleanup(&db, &[&content], &[&unlinked]).await;
}

#[tokio::test]
async fn contents_of_lists_every_content_item_a_file_is_linked_to() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content_a = seed_content(&db).await;
    let content_b = seed_content(&db).await;
    let file = seed_file(&db).await;
    repo.link(&content_a, &file, ContentFileRole::Attachment, 0)
        .await
        .expect("link a");
    repo.link(&content_b, &file, ContentFileRole::Attachment, 0)
        .await
        .expect("link b");

    let linked = repo.contents_of(&file).await.expect("list");

    assert_eq!(linked.len(), 2);
    assert!(
        repo.contents_of(&seed_file(&db).await)
            .await
            .expect("empty")
            .is_empty()
    );
    cleanup(&db, &[&content_a, &content_b], &[&file]).await;
}

#[tokio::test]
async fn deleting_the_content_removes_its_links() {
    let db = test_db_pool().await;
    let repo = links(&db);
    let content = seed_content(&db).await;
    let file = seed_file(&db).await;
    repo.link(&content, &file, ContentFileRole::Attachment, 0)
        .await
        .expect("link");

    cleanup(&db, &[&content], &[]).await;

    assert!(repo.contents_of(&file).await.expect("list").is_empty());
    cleanup(&db, &[], &[&file]).await;
}
