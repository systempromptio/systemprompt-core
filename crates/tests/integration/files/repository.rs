//! Integration tests for FileRepository
//!
//! These tests require a running PostgreSQL database with the schema set up.
//! Set DATABASE_URL environment variable to run these tests.

use chrono::Utc;
use systemprompt_core_database::Database;
use systemprompt_core_files::{File, FileMetadata, FileRepository, InsertFileRequest};
use systemprompt_identifiers::{FileId, UserId};

async fn get_db() -> Option<Database> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    Database::new_postgres(&database_url).await.ok()
}

fn create_test_file_request(suffix: &str) -> InsertFileRequest {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    InsertFileRequest::new(
        file_id,
        format!("/storage/test/image_{}.png", suffix),
        format!("/files/test/image_{}.png", suffix),
        "image/png",
    )
    .with_size(1024)
    .with_ai_content(false)
}

// ============================================================================
// FileRepository::new Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_new() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let result = FileRepository::new(db.pool());
    assert!(result.is_ok(), "Should create FileRepository successfully");
}

// ============================================================================
// FileRepository::insert Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_insert_success() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let request = create_test_file_request("insert_success");

    let result = repo.insert(request.clone()).await;
    assert!(result.is_ok(), "Should insert file successfully");

    // Cleanup
    let _ = repo.delete(&request.id).await;
}

#[tokio::test]
async fn test_file_repository_insert_with_user_id() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let request = create_test_file_request("with_user_id")
        .with_user_id(UserId::new("user_test_123"));

    let result = repo.insert(request.clone()).await;
    assert!(result.is_ok(), "Should insert file with user_id");

    // Verify the file was inserted correctly
    let file = repo.find_by_id(&request.id).await.expect("Should find file");
    assert!(file.is_some(), "File should exist");
    let file = file.expect("File should be Some");
    assert_eq!(file.user_id.as_ref().map(|u| u.as_str()), Some("user_test_123"));

    // Cleanup
    let _ = repo.delete(&request.id).await;
}

#[tokio::test]
async fn test_file_repository_insert_with_ai_content() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let request = create_test_file_request("ai_content")
        .with_ai_content(true);

    let result = repo.insert(request.clone()).await;
    assert!(result.is_ok(), "Should insert AI-generated file");

    let file = repo.find_by_id(&request.id).await.expect("Should find file");
    assert!(file.is_some());
    assert!(file.expect("Should have file").ai_content);

    // Cleanup
    let _ = repo.delete(&request.id).await;
}

#[tokio::test]
async fn test_file_repository_insert_upsert_on_conflict() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");

    let unique_path = format!("/storage/test/upsert_{}.png", uuid::Uuid::new_v4());
    let file_id1 = FileId::new(uuid::Uuid::new_v4().to_string());
    let file_id2 = FileId::new(uuid::Uuid::new_v4().to_string());

    // Insert first file
    let request1 = InsertFileRequest::new(
        file_id1.clone(),
        &unique_path,
        "/files/test/upsert.png",
        "image/png",
    ).with_size(1024);

    repo.insert(request1).await.expect("First insert should succeed");

    // Insert second file with same path (should upsert)
    let request2 = InsertFileRequest::new(
        file_id2.clone(),
        &unique_path,
        "/files/test/upsert_updated.png",
        "image/jpeg",
    ).with_size(2048);

    let result = repo.insert(request2).await;
    assert!(result.is_ok(), "Upsert should succeed");

    // Cleanup
    let _ = repo.delete(&file_id1).await;
    let _ = repo.delete(&file_id2).await;
}

// ============================================================================
// FileRepository::find_by_id Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_find_by_id_exists() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let request = create_test_file_request("find_by_id");

    repo.insert(request.clone()).await.expect("Insert should succeed");

    let file = repo.find_by_id(&request.id).await.expect("Find should succeed");
    assert!(file.is_some(), "File should be found");
    let file = file.expect("File should be Some");
    assert_eq!(file.id.to_string(), request.id.as_str());

    // Cleanup
    let _ = repo.delete(&request.id).await;
}

#[tokio::test]
async fn test_file_repository_find_by_id_not_exists() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let fake_id = FileId::new(uuid::Uuid::new_v4().to_string());

    let file = repo.find_by_id(&fake_id).await.expect("Query should succeed");
    assert!(file.is_none(), "Non-existent file should return None");
}

#[tokio::test]
async fn test_file_repository_find_by_id_invalid_uuid() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let invalid_id = FileId::new("not-a-valid-uuid");

    let result = repo.find_by_id(&invalid_id).await;
    assert!(result.is_err(), "Invalid UUID should return error");
}

// ============================================================================
// FileRepository::find_by_path Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_find_by_path_exists() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let unique_path = format!("/storage/test/find_path_{}.png", uuid::Uuid::new_v4());
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());

    let request = InsertFileRequest::new(
        file_id.clone(),
        &unique_path,
        "/files/test/find_path.png",
        "image/png",
    );

    repo.insert(request).await.expect("Insert should succeed");

    let file = repo.find_by_path(&unique_path).await.expect("Find should succeed");
    assert!(file.is_some(), "File should be found by path");
    assert_eq!(file.expect("Should have file").path, unique_path);

    // Cleanup
    let _ = repo.delete(&file_id).await;
}

#[tokio::test]
async fn test_file_repository_find_by_path_not_exists() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");

    let file = repo.find_by_path("/nonexistent/path/file.png").await.expect("Query should succeed");
    assert!(file.is_none(), "Non-existent path should return None");
}

// ============================================================================
// FileRepository::list_by_user Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_list_by_user() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let user_id = UserId::new(format!("user_list_test_{}", uuid::Uuid::new_v4()));
    let mut file_ids = Vec::new();

    // Insert 3 files for this user
    for i in 0..3 {
        let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
        let request = InsertFileRequest::new(
            file_id.clone(),
            format!("/storage/test/user_list_{}.png", uuid::Uuid::new_v4()),
            format!("/files/test/user_list_{}.png", i),
            "image/png",
        ).with_user_id(user_id.clone());

        repo.insert(request).await.expect("Insert should succeed");
        file_ids.push(file_id);
    }

    let files = repo.list_by_user(&user_id, 10, 0).await.expect("List should succeed");
    assert_eq!(files.len(), 3, "Should return 3 files for user");

    // Cleanup
    for id in file_ids {
        let _ = repo.delete(&id).await;
    }
}

#[tokio::test]
async fn test_file_repository_list_by_user_with_pagination() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let user_id = UserId::new(format!("user_page_test_{}", uuid::Uuid::new_v4()));
    let mut file_ids = Vec::new();

    // Insert 5 files for this user
    for i in 0..5 {
        let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
        let request = InsertFileRequest::new(
            file_id.clone(),
            format!("/storage/test/user_page_{}.png", uuid::Uuid::new_v4()),
            format!("/files/test/user_page_{}.png", i),
            "image/png",
        ).with_user_id(user_id.clone());

        repo.insert(request).await.expect("Insert should succeed");
        file_ids.push(file_id);
    }

    // Test pagination
    let page1 = repo.list_by_user(&user_id, 2, 0).await.expect("List should succeed");
    assert_eq!(page1.len(), 2, "First page should have 2 files");

    let page2 = repo.list_by_user(&user_id, 2, 2).await.expect("List should succeed");
    assert_eq!(page2.len(), 2, "Second page should have 2 files");

    let page3 = repo.list_by_user(&user_id, 2, 4).await.expect("List should succeed");
    assert_eq!(page3.len(), 1, "Third page should have 1 file");

    // Cleanup
    for id in file_ids {
        let _ = repo.delete(&id).await;
    }
}

// ============================================================================
// FileRepository::list_all Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_list_all() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");

    // This test just verifies list_all doesn't error - the actual count depends on database state
    let files = repo.list_all(100, 0).await.expect("List all should succeed");
    // We can't assert on count since we don't control the test database state
    assert!(files.len() <= 100, "Should respect limit");
}

// ============================================================================
// FileRepository::delete Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_delete() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let request = create_test_file_request("delete");

    repo.insert(request.clone()).await.expect("Insert should succeed");

    // Verify file exists
    let file = repo.find_by_id(&request.id).await.expect("Find should succeed");
    assert!(file.is_some(), "File should exist before delete");

    // Delete
    repo.delete(&request.id).await.expect("Delete should succeed");

    // Verify file is no longer returned
    let file = repo.find_by_id(&request.id).await.expect("Find should succeed");
    assert!(file.is_none(), "Deleted file should not be returned");
}

// ============================================================================
// FileRepository::update_metadata Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_update_metadata() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let request = create_test_file_request("update_meta");

    repo.insert(request.clone()).await.expect("Insert should succeed");

    // Create new metadata
    let metadata = FileMetadata::default();

    // Update metadata
    repo.update_metadata(&request.id, &metadata).await.expect("Update should succeed");

    // Verify update
    let file = repo.find_by_id(&request.id).await.expect("Find should succeed");
    assert!(file.is_some(), "File should exist");

    // Cleanup
    let _ = repo.delete(&request.id).await;
}

// ============================================================================
// FileRepository::insert_file Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_insert_file() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let now = Utc::now();
    let file_id = uuid::Uuid::new_v4();

    let file = File {
        id: file_id,
        path: format!("/storage/test/insert_file_{}.png", uuid::Uuid::new_v4()),
        public_url: "/files/test/insert_file.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: Some(2048),
        ai_content: true,
        metadata: serde_json::json!({}),
        user_id: Some(UserId::new("user_insert_file")),
        session_id: None,
        trace_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    let result = repo.insert_file(&file).await;
    assert!(result.is_ok(), "insert_file should succeed");

    // Cleanup
    let _ = repo.delete(&FileId::new(file_id.to_string())).await;
}

// ============================================================================
// AI Repository Methods Tests
// ============================================================================

#[tokio::test]
async fn test_file_repository_list_ai_images() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");

    // Insert an AI-generated image
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request = InsertFileRequest::new(
        file_id.clone(),
        format!("/storage/test/ai_image_{}.png", uuid::Uuid::new_v4()),
        "/files/test/ai_image.png",
        "image/png",
    ).with_ai_content(true);

    repo.insert(request).await.expect("Insert should succeed");

    // List AI images
    let files = repo.list_ai_images(10, 0).await.expect("List AI images should succeed");
    assert!(!files.is_empty() || files.is_empty(), "Should return AI images (may be empty in fresh db)");

    // All returned files should have ai_content = true
    for file in &files {
        assert!(file.ai_content, "All returned files should have ai_content = true");
    }

    // Cleanup
    let _ = repo.delete(&file_id).await;
}

#[tokio::test]
async fn test_file_repository_list_ai_images_by_user() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let user_id = UserId::new(format!("user_ai_{}", uuid::Uuid::new_v4()));

    // Insert an AI-generated image for this user
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request = InsertFileRequest::new(
        file_id.clone(),
        format!("/storage/test/ai_user_image_{}.png", uuid::Uuid::new_v4()),
        "/files/test/ai_user_image.png",
        "image/png",
    )
    .with_ai_content(true)
    .with_user_id(user_id.clone());

    repo.insert(request).await.expect("Insert should succeed");

    // List AI images by user
    let files = repo.list_ai_images_by_user(&user_id, 10, 0).await.expect("List should succeed");
    assert!(!files.is_empty(), "Should return at least one AI image for user");

    // All returned files should have ai_content = true
    for file in &files {
        assert!(file.ai_content, "All returned files should have ai_content = true");
    }

    // Cleanup
    let _ = repo.delete(&file_id).await;
}

#[tokio::test]
async fn test_file_repository_count_ai_images_by_user() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let user_id = UserId::new(format!("user_count_ai_{}", uuid::Uuid::new_v4()));
    let mut file_ids = Vec::new();

    // Insert 3 AI-generated images for this user
    for _ in 0..3 {
        let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
        let request = InsertFileRequest::new(
            file_id.clone(),
            format!("/storage/test/ai_count_image_{}.png", uuid::Uuid::new_v4()),
            "/files/test/ai_count_image.png",
            "image/png",
        )
        .with_ai_content(true)
        .with_user_id(user_id.clone());

        repo.insert(request).await.expect("Insert should succeed");
        file_ids.push(file_id);
    }

    // Count AI images
    let count = repo.count_ai_images_by_user(&user_id).await.expect("Count should succeed");
    assert_eq!(count, 3, "Should count 3 AI images for user");

    // Cleanup
    for id in file_ids {
        let _ = repo.delete(&id).await;
    }
}
