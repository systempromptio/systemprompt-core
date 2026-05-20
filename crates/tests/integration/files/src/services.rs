//! Integration tests for FileRepository AI/content surface.
//!
//! These tests require a running PostgreSQL database with the schema set up.
//! Set DATABASE_URL environment variable to run these tests.

use systemprompt_database::Database;
use systemprompt_files::{FileRepository, InsertFileRequest};
use systemprompt_identifiers::{ContentId, FileId, UserId};

async fn get_db() -> Option<std::sync::Arc<Database>> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    Database::new_postgres(&database_url).await.ok().map(std::sync::Arc::new)
}

fn create_test_file_request(suffix: &str) -> InsertFileRequest {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    InsertFileRequest::new(
        file_id,
        format!("/storage/test/svc_image_{}.png", suffix),
        format!("/files/test/svc_image_{}.png", suffix),
        "image/png",
    )
    .with_size(1024)
}

#[tokio::test]
async fn test_repository_new() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let result = FileRepository::new(&db);
    assert!(result.is_ok(), "FileRepository::new should succeed");
}

#[tokio::test]
async fn test_repository_insert() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(&db).expect("Failed to create repository");
    let request = create_test_file_request(&uuid::Uuid::new_v4().to_string());

    let result = repo.insert(request.clone()).await;
    assert!(result.is_ok(), "FileRepository::insert should succeed");

    let file = repo
        .find_by_id(&request.id)
        .await
        .expect("should query inserted file")
        .expect("inserted file should exist");
    assert_eq!(
        file.id.to_string(),
        request.id.as_str(),
        "file id should match"
    );
    assert_eq!(file.mime_type, "image/png", "mime type should match");
    assert_eq!(file.size_bytes, Some(1024), "size should match");

    let _ = repo.delete(&request.id).await;
}

#[tokio::test]
async fn test_repository_find_by_path() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(&db).expect("Failed to create repository");
    let unique_suffix = uuid::Uuid::new_v4().to_string();
    let request = create_test_file_request(&unique_suffix);
    let path = request.path.clone();

    repo.insert(request.clone())
        .await
        .expect("Insert should succeed");

    let file = repo.find_by_path(&path).await.expect("Find should succeed");
    let file = file.expect("file should be found by path");
    assert_eq!(file.path, path, "returned file path should match query path");
    assert_eq!(
        file.id.to_string(),
        request.id.as_str(),
        "file id should match"
    );

    let _ = repo.delete(&request.id).await;
}

#[tokio::test]
async fn test_repository_list_by_user() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(&db).expect("Failed to create repository");
    let user_id = UserId::new(format!("svc_user_{}", uuid::Uuid::new_v4()));
    let mut file_ids = Vec::new();

    for _ in 0..2 {
        let request = create_test_file_request(&uuid::Uuid::new_v4().to_string())
            .with_user_id(user_id.clone());

        repo.insert(request.clone())
            .await
            .expect("Insert should succeed");
        file_ids.push(request.id);
    }

    let files = repo
        .list_by_user(&user_id, 10, 0)
        .await
        .expect("List should succeed");
    assert_eq!(files.len(), 2, "Should return 2 files for user");
    for file in &files {
        assert_eq!(
            file.user_id.as_ref().map(|u| u.as_str()),
            Some(user_id.as_str()),
            "all files should belong to the test user"
        );
    }

    for id in file_ids {
        let _ = repo.delete(&id).await;
    }
}

#[tokio::test]
async fn test_repository_delete() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(&db).expect("Failed to create repository");
    let request = create_test_file_request(&uuid::Uuid::new_v4().to_string());

    repo.insert(request.clone())
        .await
        .expect("Insert should succeed");

    repo.delete(&request.id)
        .await
        .expect("Delete should succeed");

    let file = repo.find_by_id(&request.id).await.expect("Find should succeed");
    assert!(file.is_none(), "File should be deleted");
}

#[tokio::test]
async fn test_repository_list_ai_images() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(&db).expect("Failed to create repository");

    let request =
        create_test_file_request(&uuid::Uuid::new_v4().to_string()).with_ai_content(true);

    repo.insert(request.clone())
        .await
        .expect("Insert should succeed");

    let images = repo
        .list_ai_images(10, 0)
        .await
        .expect("List should succeed");
    assert!(
        !images.is_empty(),
        "should return at least the AI image we inserted"
    );
    for img in &images {
        assert!(img.ai_content, "All returned images should be AI content");
    }

    let _ = repo.delete(&request.id).await;
}

#[tokio::test]
async fn test_repository_count_ai_images_by_user() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(&db).expect("Failed to create repository");
    let user_id = UserId::new(format!("ai_count_user_{}", uuid::Uuid::new_v4()));

    let mut file_ids = Vec::new();
    for _ in 0..2 {
        let request = create_test_file_request(&uuid::Uuid::new_v4().to_string())
            .with_ai_content(true)
            .with_user_id(user_id.clone());

        repo.insert(request.clone())
            .await
            .expect("Insert should succeed");
        file_ids.push(request.id);
    }

    let count = repo
        .count_ai_images_by_user(&user_id)
        .await
        .expect("Count should succeed");
    assert_eq!(count, 2, "Should count 2 AI images");

    for id in file_ids {
        let _ = repo.delete(&id).await;
    }
}

#[tokio::test]
async fn test_repository_list_files_by_content() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(&db).expect("Failed to create repository");

    let content_id = ContentId::new(format!("list_test_{}", uuid::Uuid::new_v4()));

    let files = repo
        .list_files_by_content(&content_id)
        .await
        .expect("List should succeed");
    assert!(
        files.is_empty(),
        "Should return empty list for non-existent content"
    );
}
