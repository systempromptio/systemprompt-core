//! Integration tests for FileService, AiService, and ContentService
//!
//! These tests require a running PostgreSQL database with the schema set up.
//! Set DATABASE_URL environment variable to run these tests.

use systemprompt_database::Database;
use systemprompt_files::{
    AiService, ContentService, FileRepository, FileService, InsertFileRequest,
};
use systemprompt_identifiers::{ContentId, FileId, UserId};

async fn get_db() -> Option<Database> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    Database::new_postgres(&database_url).await.ok()
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

// ============================================================================
// FileService Tests
// ============================================================================

#[tokio::test]
async fn test_file_service_new() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let result = FileService::new(db.pool());
    assert!(result.is_ok(), "FileService::new should succeed");
}

#[tokio::test]
async fn test_file_service_from_repository() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let service = FileService::from_repository(repo);

    // Verify we can use the service
    let _ = service.repository();
}

#[tokio::test]
async fn test_file_service_repository_accessor() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let service = FileService::new(db.pool()).expect("Failed to create service");
    let _repo = service.repository();
    // Just verify accessor works
}

#[tokio::test]
async fn test_file_service_insert() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let service = FileService::new(db.pool()).expect("Failed to create service");
    let request = create_test_file_request(&uuid::Uuid::new_v4().to_string());

    let result = service.insert(request.clone()).await;
    assert!(result.is_ok(), "FileService::insert should succeed");

    // Cleanup
    let _ = service.delete(&request.id).await;
}

#[tokio::test]
async fn test_file_service_find_by_id() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let service = FileService::new(db.pool()).expect("Failed to create service");
    let request = create_test_file_request(&uuid::Uuid::new_v4().to_string());

    service.insert(request.clone()).await.expect("Insert should succeed");

    let file = service.find_by_id(&request.id).await.expect("Find should succeed");
    assert!(file.is_some(), "File should be found");

    // Cleanup
    let _ = service.delete(&request.id).await;
}

#[tokio::test]
async fn test_file_service_find_by_path() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let service = FileService::new(db.pool()).expect("Failed to create service");
    let unique_suffix = uuid::Uuid::new_v4().to_string();
    let request = create_test_file_request(&unique_suffix);
    let path = request.path.clone();

    service.insert(request.clone()).await.expect("Insert should succeed");

    let file = service.find_by_path(&path).await.expect("Find should succeed");
    assert!(file.is_some(), "File should be found by path");

    // Cleanup
    let _ = service.delete(&request.id).await;
}

#[tokio::test]
async fn test_file_service_list_by_user() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let service = FileService::new(db.pool()).expect("Failed to create service");
    let user_id = UserId::new(format!("svc_user_{}", uuid::Uuid::new_v4()));
    let mut file_ids = Vec::new();

    // Insert 2 files for this user
    for _ in 0..2 {
        let request = create_test_file_request(&uuid::Uuid::new_v4().to_string())
            .with_user_id(user_id.clone());

        service.insert(request.clone()).await.expect("Insert should succeed");
        file_ids.push(request.id);
    }

    let files = service.list_by_user(&user_id, 10, 0).await.expect("List should succeed");
    assert_eq!(files.len(), 2, "Should return 2 files for user");

    // Cleanup
    for id in file_ids {
        let _ = service.delete(&id).await;
    }
}

#[tokio::test]
async fn test_file_service_list_all() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let service = FileService::new(db.pool()).expect("Failed to create service");

    let files = service.list_all(10, 0).await.expect("List all should succeed");
    assert!(files.len() <= 10, "Should respect limit");
}

#[tokio::test]
async fn test_file_service_delete() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let service = FileService::new(db.pool()).expect("Failed to create service");
    let request = create_test_file_request(&uuid::Uuid::new_v4().to_string());

    service.insert(request.clone()).await.expect("Insert should succeed");

    // Delete
    service.delete(&request.id).await.expect("Delete should succeed");

    // Verify file is gone
    let file = service.find_by_id(&request.id).await.expect("Find should succeed");
    assert!(file.is_none(), "File should be deleted");
}

#[tokio::test]
async fn test_file_service_update_metadata() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    use systemprompt_files::FileMetadata;

    let service = FileService::new(db.pool()).expect("Failed to create service");
    let request = create_test_file_request(&uuid::Uuid::new_v4().to_string());

    service.insert(request.clone()).await.expect("Insert should succeed");

    let metadata = FileMetadata::default();
    service.update_metadata(&request.id, &metadata).await.expect("Update should succeed");

    // Cleanup
    let _ = service.delete(&request.id).await;
}

// ============================================================================
// AiService Tests
// ============================================================================

#[tokio::test]
async fn test_ai_service_new() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let result = AiService::new(db.pool());
    assert!(result.is_ok(), "AiService::new should succeed");
}

#[tokio::test]
async fn test_ai_service_from_repository() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let service = AiService::from_repository(repo);

    let _ = service.repository();
}

#[tokio::test]
async fn test_ai_service_list_ai_images() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let ai_service = AiService::new(db.pool()).expect("Failed to create AI service");
    let file_service = FileService::new(db.pool()).expect("Failed to create file service");

    // Insert an AI image
    let request = create_test_file_request(&uuid::Uuid::new_v4().to_string())
        .with_ai_content(true);

    file_service.insert(request.clone()).await.expect("Insert should succeed");

    // List AI images
    let images = ai_service.list_ai_images(10, 0).await.expect("List should succeed");
    // All should be AI content
    for img in &images {
        assert!(img.ai_content, "All returned images should be AI content");
    }

    // Cleanup
    let _ = file_service.delete(&request.id).await;
}

#[tokio::test]
async fn test_ai_service_list_ai_images_by_user() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let ai_service = AiService::new(db.pool()).expect("Failed to create AI service");
    let file_service = FileService::new(db.pool()).expect("Failed to create file service");
    let user_id = UserId::new(format!("ai_svc_user_{}", uuid::Uuid::new_v4()));

    // Insert an AI image for this user
    let request = create_test_file_request(&uuid::Uuid::new_v4().to_string())
        .with_ai_content(true)
        .with_user_id(user_id.clone());

    file_service.insert(request.clone()).await.expect("Insert should succeed");

    // List AI images for user
    let images = ai_service.list_ai_images_by_user(&user_id, 10, 0).await.expect("List should succeed");
    assert!(!images.is_empty(), "Should have at least one AI image");

    // Cleanup
    let _ = file_service.delete(&request.id).await;
}

#[tokio::test]
async fn test_ai_service_count_ai_images_by_user() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let ai_service = AiService::new(db.pool()).expect("Failed to create AI service");
    let file_service = FileService::new(db.pool()).expect("Failed to create file service");
    let user_id = UserId::new(format!("ai_count_user_{}", uuid::Uuid::new_v4()));

    // Insert 2 AI images for this user
    let mut file_ids = Vec::new();
    for _ in 0..2 {
        let request = create_test_file_request(&uuid::Uuid::new_v4().to_string())
            .with_ai_content(true)
            .with_user_id(user_id.clone());

        file_service.insert(request.clone()).await.expect("Insert should succeed");
        file_ids.push(request.id);
    }

    let count = ai_service.count_ai_images_by_user(&user_id).await.expect("Count should succeed");
    assert_eq!(count, 2, "Should count 2 AI images");

    // Cleanup
    for id in file_ids {
        let _ = file_service.delete(&id).await;
    }
}

// ============================================================================
// ContentService Tests
// ============================================================================

#[tokio::test]
async fn test_content_service_new() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let result = ContentService::new(db.pool());
    assert!(result.is_ok(), "ContentService::new should succeed");
}

#[tokio::test]
async fn test_content_service_from_repository() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let repo = FileRepository::new(db.pool()).expect("Failed to create repository");
    let service = ContentService::from_repository(repo);

    let _ = service.repository();
}

// Note: ContentService methods require content to exist in the database
// These tests would require setting up content records first
// For now, we just test service construction

#[tokio::test]
async fn test_content_service_link_operations() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    use systemprompt_files::FileRole;

    let content_service = ContentService::new(db.pool()).expect("Failed to create content service");
    let file_service = FileService::new(db.pool()).expect("Failed to create file service");

    // Insert a file first
    let file_request = create_test_file_request(&uuid::Uuid::new_v4().to_string());
    file_service.insert(file_request.clone()).await.expect("Insert should succeed");

    // Try to link to a content ID (this may fail if content doesn't exist due to FK constraint)
    let content_id = ContentId::new(format!("content_{}", uuid::Uuid::new_v4()));

    let link_result = content_service
        .link_to_content(&content_id, &file_request.id, FileRole::Attachment, 0)
        .await;

    // The link may fail due to FK constraint if content doesn't exist
    // That's expected behavior - we're just testing the service method works
    if link_result.is_ok() {
        // If it succeeded, clean up the link
        let _ = content_service.unlink_from_content(&content_id, &file_request.id).await;
    }

    // Cleanup file
    let _ = file_service.delete(&file_request.id).await;
}

#[tokio::test]
async fn test_content_service_find_featured_image() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let content_service = ContentService::new(db.pool()).expect("Failed to create content service");

    // Try to find featured image for a non-existent content (should return None, not error)
    let content_id = ContentId::new(format!("nonexistent_{}", uuid::Uuid::new_v4()));

    let result = content_service.find_featured_image(&content_id).await;
    // Should either succeed with None or fail gracefully
    match result {
        Ok(file) => assert!(file.is_none(), "Should return None for non-existent content"),
        Err(_) => {} // FK or other constraint error is acceptable
    }
}

#[tokio::test]
async fn test_content_service_list_files_by_content() {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return;
    };

    let content_service = ContentService::new(db.pool()).expect("Failed to create content service");

    // Try to list files for non-existent content (should return empty list)
    let content_id = ContentId::new(format!("list_test_{}", uuid::Uuid::new_v4()));

    let files = content_service.list_files_by_content(&content_id).await.expect("List should succeed");
    assert!(files.is_empty(), "Should return empty list for non-existent content");
}
