//! DB-backed tests for the file/content association queries on
//! [`FileRepository`] (`link_to_content`, `unlink_from_content`,
//! `list_files_by_content`, `find_featured_image`, `set_featured`,
//! `list_content_by_file`).
//!
//! Each test seeds its own `files` and `markdown_content` parents with unique
//! ids and removes them on completion so parallel runs do not collide.

use systemprompt_database::DbPool;
use systemprompt_files::{FileRepository, FileRole, InsertFileRequest};
use systemprompt_identifiers::{ContentId, ContextId, FileId};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn new_file_id() -> FileId {
    FileId::new(uuid::Uuid::new_v4().to_string())
}

fn new_content_id() -> ContentId {
    ContentId::new(format!("ct-test-{}", uuid::Uuid::new_v4().simple()))
}

async fn seed_file(repo: &FileRepository, id: &FileId) {
    let request = InsertFileRequest::new(
        id.clone(),
        format!("/storage/test/{}.png", id.as_str()),
        format!("/files/test/{}.png", id.as_str()),
        "image/png",
    )
    .with_size(128)
    .with_context_id(ContextId::generate());
    repo.insert(request).await.expect("seed file");
}

async fn seed_content(pool: &DbPool, id: &ContentId) {
    let p = pool.pool_arc().expect("read pool");
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
    .execute(p.as_ref())
    .await
    .expect("seed markdown_content");
}

async fn cleanup_content(pool: &DbPool, id: &ContentId) {
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("DELETE FROM markdown_content WHERE id = $1")
        .bind(id.as_str())
        .execute(p.as_ref())
        .await
        .ok();
}

async fn cleanup_file(repo: &FileRepository, id: &FileId) {
    repo.delete(id).await.ok();
}

#[tokio::test]
async fn link_to_content_persists_association() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let file_id = new_file_id();
    seed_content(&db, &content_id).await;
    seed_file(&repo, &file_id).await;

    let link = repo
        .link_to_content(&content_id, &file_id, FileRole::Attachment, 3)
        .await
        .expect("link");

    assert_eq!(link.content_id.as_str(), content_id.as_str());
    assert_eq!(link.role, FileRole::Attachment.as_str());
    assert_eq!(link.display_order, 3);

    cleanup_file(&repo, &file_id).await;
    cleanup_content(&db, &content_id).await;
}

#[tokio::test]
async fn link_to_content_upserts_display_order_on_conflict() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let file_id = new_file_id();
    seed_content(&db, &content_id).await;
    seed_file(&repo, &file_id).await;

    repo.link_to_content(&content_id, &file_id, FileRole::Attachment, 1)
        .await
        .expect("first link");
    let second = repo
        .link_to_content(&content_id, &file_id, FileRole::Attachment, 9)
        .await
        .expect("conflict upsert");

    assert_eq!(second.display_order, 9, "display_order updated on conflict");

    let links = repo.list_content_by_file(&file_id).await.expect("list");
    assert_eq!(links.len(), 1, "upsert must not create a duplicate row");

    cleanup_file(&repo, &file_id).await;
    cleanup_content(&db, &content_id).await;
}

#[tokio::test]
async fn link_to_content_rejects_non_uuid_file_id() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let bad = FileId::new("not-a-uuid");
    let err = repo
        .link_to_content(&content_id, &bad, FileRole::Attachment, 0)
        .await
        .expect_err("non-uuid file id must fail");
    assert!(err.to_string().contains("Invalid UUID"));
}

#[tokio::test]
async fn unlink_removes_association() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let file_id = new_file_id();
    seed_content(&db, &content_id).await;
    seed_file(&repo, &file_id).await;
    repo.link_to_content(&content_id, &file_id, FileRole::Attachment, 0)
        .await
        .expect("link");

    repo.unlink_from_content(&content_id, &file_id)
        .await
        .expect("unlink");

    let links = repo.list_content_by_file(&file_id).await.expect("list");
    assert!(links.is_empty(), "association removed");

    cleanup_file(&repo, &file_id).await;
    cleanup_content(&db, &content_id).await;
}

#[tokio::test]
async fn unlink_rejects_non_uuid_file_id() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let bad = FileId::new("xyz");
    let err = repo
        .unlink_from_content(&content_id, &bad)
        .await
        .expect_err("non-uuid must fail");
    assert!(err.to_string().contains("Invalid UUID"));
}

#[tokio::test]
async fn list_files_by_content_returns_ordered_joined_rows() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let first = new_file_id();
    let second = new_file_id();
    seed_content(&db, &content_id).await;
    seed_file(&repo, &first).await;
    seed_file(&repo, &second).await;

    repo.link_to_content(&content_id, &second, FileRole::Attachment, 5)
        .await
        .expect("link second");
    repo.link_to_content(&content_id, &first, FileRole::Featured, 1)
        .await
        .expect("link first");

    let rows = repo
        .list_files_by_content(&content_id)
        .await
        .expect("list files");

    assert_eq!(rows.len(), 2);
    let (file0, cf0) = &rows[0];
    assert_eq!(
        cf0.display_order, 1,
        "results ordered by display_order ascending"
    );
    assert_eq!(file0.id.to_string(), first.as_str());
    assert_eq!(cf0.content_id.as_str(), content_id.as_str());
    let (_, cf1) = &rows[1];
    assert_eq!(cf1.display_order, 5);

    cleanup_file(&repo, &first).await;
    cleanup_file(&repo, &second).await;
    cleanup_content(&db, &content_id).await;
}

#[tokio::test]
async fn list_files_by_content_empty_for_unknown_content() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let rows = repo.list_files_by_content(&content_id).await.expect("list");
    assert!(rows.is_empty());
}

#[tokio::test]
async fn find_featured_image_returns_only_featured() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let attachment = new_file_id();
    let featured = new_file_id();
    seed_content(&db, &content_id).await;
    seed_file(&repo, &attachment).await;
    seed_file(&repo, &featured).await;

    repo.link_to_content(&content_id, &attachment, FileRole::Attachment, 0)
        .await
        .expect("link attachment");
    repo.link_to_content(&content_id, &featured, FileRole::Featured, 0)
        .await
        .expect("link featured");

    let found = repo
        .find_featured_image(&content_id)
        .await
        .expect("find featured")
        .expect("featured present");
    assert_eq!(found.id.to_string(), featured.as_str());

    cleanup_file(&repo, &attachment).await;
    cleanup_file(&repo, &featured).await;
    cleanup_content(&db, &content_id).await;
}

#[tokio::test]
async fn find_featured_image_none_when_only_attachments() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let attachment = new_file_id();
    seed_content(&db, &content_id).await;
    seed_file(&repo, &attachment).await;
    repo.link_to_content(&content_id, &attachment, FileRole::Attachment, 0)
        .await
        .expect("link");

    let found = repo
        .find_featured_image(&content_id)
        .await
        .expect("find featured");
    assert!(found.is_none());

    cleanup_file(&repo, &attachment).await;
    cleanup_content(&db, &content_id).await;
}

#[tokio::test]
async fn set_featured_demotes_existing_and_promotes_target() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let old_featured = new_file_id();
    let new_featured = new_file_id();
    seed_content(&db, &content_id).await;
    seed_file(&repo, &old_featured).await;
    seed_file(&repo, &new_featured).await;

    repo.link_to_content(&content_id, &old_featured, FileRole::Featured, 0)
        .await
        .expect("link old featured");
    repo.link_to_content(&content_id, &new_featured, FileRole::Attachment, 1)
        .await
        .expect("link new as attachment");

    repo.set_featured(&new_featured, &content_id)
        .await
        .expect("set featured");

    let featured = repo
        .find_featured_image(&content_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(
        featured.id.to_string(),
        new_featured.as_str(),
        "new file is now featured"
    );

    let old_links = repo
        .list_content_by_file(&old_featured)
        .await
        .expect("list old");
    assert_eq!(
        old_links[0].role,
        FileRole::Attachment.as_str(),
        "previous featured demoted to attachment"
    );

    cleanup_file(&repo, &old_featured).await;
    cleanup_file(&repo, &new_featured).await;
    cleanup_content(&db, &content_id).await;
}

#[tokio::test]
async fn set_featured_errors_when_file_not_linked() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let unlinked = new_file_id();
    seed_content(&db, &content_id).await;
    seed_file(&repo, &unlinked).await;

    let err = repo
        .set_featured(&unlinked, &content_id)
        .await
        .expect_err("unlinked file cannot be featured");
    assert!(err.to_string().contains("not linked"));

    cleanup_file(&repo, &unlinked).await;
    cleanup_content(&db, &content_id).await;
}

#[tokio::test]
async fn set_featured_rejects_non_uuid_file_id() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_id = new_content_id();
    let err = repo
        .set_featured(&FileId::new("bad"), &content_id)
        .await
        .expect_err("non-uuid must fail");
    assert!(err.to_string().contains("Invalid UUID"));
}

#[tokio::test]
async fn list_content_by_file_returns_links() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let content_a = new_content_id();
    let content_b = new_content_id();
    let file_id = new_file_id();
    seed_content(&db, &content_a).await;
    seed_content(&db, &content_b).await;
    seed_file(&repo, &file_id).await;

    repo.link_to_content(&content_a, &file_id, FileRole::Attachment, 0)
        .await
        .expect("link a");
    repo.link_to_content(&content_b, &file_id, FileRole::Attachment, 0)
        .await
        .expect("link b");

    let links = repo.list_content_by_file(&file_id).await.expect("list");
    assert_eq!(links.len(), 2);
    assert!(
        links
            .iter()
            .all(|l| l.file_id.to_string() == file_id.as_str())
    );

    cleanup_file(&repo, &file_id).await;
    cleanup_content(&db, &content_a).await;
    cleanup_content(&db, &content_b).await;
}

#[tokio::test]
async fn list_content_by_file_rejects_non_uuid() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let err = repo
        .list_content_by_file(&FileId::new("nope"))
        .await
        .expect_err("non-uuid must fail");
    assert!(err.to_string().contains("Invalid UUID"));
}

#[tokio::test]
async fn list_content_by_file_empty_for_unknown_file() {
    let Some(db) = db().await else { return };
    let repo = FileRepository::new(&db).expect("repo");
    let file_id = new_file_id();
    let links = repo.list_content_by_file(&file_id).await.expect("list");
    assert!(links.is_empty());
}
