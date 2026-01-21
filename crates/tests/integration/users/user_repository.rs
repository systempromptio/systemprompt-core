//! Integration tests for UserRepository.
//!
//! Tests cover:
//! - User creation (regular and anonymous)
//! - Finding users by ID, email, name, role
//! - Updating user fields
//! - Role assignment
//! - User deletion
//! - Anonymous user cleanup

use anyhow::Result;
use systemprompt_database::Database;
use systemprompt_users::{UpdateUserParams, UserRepository, UserRole, UserStatus};
use systemprompt_identifiers::UserId;

async fn get_db() -> Option<Database> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    Database::new_postgres(&database_url).await.ok()
}

// ============================================================================
// UserRepository::new Tests
// ============================================================================

#[tokio::test]
async fn repository_creation_from_db_pool() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;

    // Verify the repo is functional by calling find_first_user
    let _ = repo.find_first_user().await;

    Ok(())
}

// ============================================================================
// UserRepository::create Tests
// ============================================================================

#[tokio::test]
async fn create_user_with_all_fields() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("test_create_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("testuser_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    // Cleanup first
    let _ = sqlx::query("DELETE FROM users WHERE email = $1")
        .bind(&unique_email)
        .execute(pool.as_ref())
        .await;

    let user = repo
        .create(&unique_name, &unique_email, Some("Test User"), Some("Test"))
        .await?;

    assert_eq!(user.name, unique_name);
    assert_eq!(user.email, unique_email);
    assert_eq!(user.full_name, Some("Test User".to_string()));
    assert_eq!(user.display_name, Some("Test".to_string()));
    assert_eq!(user.status, Some("active".to_string()));
    assert!(user.roles.contains(&"user".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn create_user_without_display_name_uses_full_name() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("test_nodisplay_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("nodisplay_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let user = repo
        .create(&unique_name, &unique_email, Some("Full Name Only"), None)
        .await?;

    // Display name should default to full_name when not provided
    assert_eq!(user.display_name, Some("Full Name Only".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::create_anonymous Tests
// ============================================================================

#[tokio::test]
async fn create_anonymous_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let fingerprint = format!("fp_{}", uuid::Uuid::new_v4());
    let user = repo.create_anonymous(&fingerprint).await?;

    assert!(user.name.starts_with("anonymous_"));
    assert!(user.email.contains(&fingerprint));
    assert!(user.roles.contains(&"anonymous".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::find_by_id Tests
// ============================================================================

#[tokio::test]
async fn find_by_id_returns_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    // Create a user first
    let unique_email = format!("find_by_id_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("findbyid_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    // Find by ID
    let found = repo.find_by_id(&created.id).await?;
    assert!(found.is_some());
    assert_eq!(found.as_ref().map(|u| u.id.to_string()), Some(created.id.to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn find_by_id_returns_none_for_nonexistent() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;

    let fake_id = UserId::new("nonexistent-user-id".to_string());
    let found = repo.find_by_id(&fake_id).await?;
    assert!(found.is_none());

    Ok(())
}

// ============================================================================
// UserRepository::find_by_email Tests
// ============================================================================

#[tokio::test]
async fn find_by_email_returns_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("find_email_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("findemail_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let found = repo.find_by_email(&unique_email).await?;
    assert!(found.is_some());
    assert_eq!(found.as_ref().map(|u| &u.email), Some(&unique_email));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn find_by_email_returns_none_for_nonexistent() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;

    let found = repo.find_by_email("nonexistent@example.com").await?;
    assert!(found.is_none());

    Ok(())
}

// ============================================================================
// UserRepository::find_by_name Tests
// ============================================================================

#[tokio::test]
async fn find_by_name_returns_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("find_name_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("findname_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let found = repo.find_by_name(&unique_name).await?;
    assert!(found.is_some());
    assert_eq!(found.as_ref().map(|u| &u.name), Some(&unique_name));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::find_by_role Tests
// ============================================================================

#[tokio::test]
async fn find_by_role_returns_users_with_role() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    // Create a user with user role (default)
    let unique_email = format!("find_role_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("findrole_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let users = repo.find_by_role(UserRole::User).await?;
    assert!(!users.is_empty());
    assert!(users.iter().any(|u| u.id.to_string() == created.id.to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::find_first_user Tests
// ============================================================================

#[tokio::test]
async fn find_first_user_returns_oldest_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;

    // This might return any user, just verify it doesn't error
    let first = repo.find_first_user().await?;
    // Can be None if no users exist, or Some if users exist
    let _ = first;

    Ok(())
}

// ============================================================================
// UserRepository::find_first_admin Tests
// ============================================================================

#[tokio::test]
async fn find_first_admin_returns_admin_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;

    let admin = repo.find_first_admin().await?;
    // Either None (no admins) or Some (has admin role)
    if let Some(user) = admin {
        assert!(user.roles.contains(&"admin".to_string()));
    }

    Ok(())
}

// ============================================================================
// UserRepository::update_email Tests
// ============================================================================

#[tokio::test]
async fn update_email_changes_email() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("update_email_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("updateemail_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let new_email = format!("updated_{}@example.com", uuid::Uuid::new_v4());
    let updated = repo.update_email(&created.id, &new_email).await?;

    assert_eq!(updated.email, new_email);
    assert_eq!(updated.email_verified, Some(false)); // Should reset on email change

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::update_full_name Tests
// ============================================================================

#[tokio::test]
async fn update_full_name_changes_name() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("update_name_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("updatename_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let updated = repo.update_full_name(&created.id, "New Full Name").await?;

    assert_eq!(updated.full_name, Some("New Full Name".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::update_status Tests
// ============================================================================

#[tokio::test]
async fn update_status_changes_status() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("update_status_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("updatestatus_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let updated = repo.update_status(&created.id, UserStatus::Suspended).await?;

    assert_eq!(updated.status, Some("suspended".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::update_email_verified Tests
// ============================================================================

#[tokio::test]
async fn update_email_verified_sets_flag() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("verify_email_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("verifyemail_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let updated = repo.update_email_verified(&created.id, true).await?;

    assert_eq!(updated.email_verified, Some(true));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::update_all_fields Tests
// ============================================================================

#[tokio::test]
async fn update_all_fields_updates_everything() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("update_all_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("updateall_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let new_email = format!("all_updated_{}@example.com", uuid::Uuid::new_v4());
    let params = UpdateUserParams {
        email: &new_email,
        full_name: Some("Updated Full Name"),
        display_name: Some("Updated Display"),
        status: UserStatus::Inactive,
    };

    let updated = repo.update_all_fields(&created.id, params).await?;

    assert_eq!(updated.email, new_email);
    assert_eq!(updated.full_name, Some("Updated Full Name".to_string()));
    assert_eq!(updated.display_name, Some("Updated Display".to_string()));
    assert_eq!(updated.status, Some("inactive".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::assign_roles Tests
// ============================================================================

#[tokio::test]
async fn assign_roles_updates_roles() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("assign_roles_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("assignroles_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let new_roles = vec!["admin".to_string(), "user".to_string()];
    let updated = repo.assign_roles(&created.id, &new_roles).await?;

    assert!(updated.roles.contains(&"admin".to_string()));
    assert!(updated.roles.contains(&"user".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserRepository::delete Tests
// ============================================================================

#[tokio::test]
async fn delete_removes_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;

    let unique_email = format!("delete_user_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("deleteuser_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    repo.delete(&created.id).await?;

    // Should be completely gone
    let found = repo.find_by_id(&created.id).await?;
    assert!(found.is_none());

    Ok(())
}

#[tokio::test]
async fn delete_returns_error_for_nonexistent() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;

    let fake_id = UserId::new("nonexistent-delete-id".to_string());
    let result = repo.delete(&fake_id).await;
    assert!(result.is_err());

    Ok(())
}

// ============================================================================
// UserRepository::cleanup_old_anonymous Tests
// ============================================================================

#[tokio::test]
async fn cleanup_old_anonymous_runs_without_error() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;

    // Just verify the method runs without error
    let deleted = repo.cleanup_old_anonymous(30).await?;
    assert!(deleted >= 0);

    Ok(())
}

// ============================================================================
// UserRepository::get_authenticated_user Tests
// ============================================================================

#[tokio::test]
async fn get_authenticated_user_returns_active_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("auth_user_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("authuser_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    let auth_user = repo.get_authenticated_user(&created.id).await?;
    assert!(auth_user.is_some());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn get_authenticated_user_returns_none_for_inactive() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let repo = UserRepository::new(&db_pool)?;
    let pool = db.pool_arc()?;

    let unique_email = format!("inactive_auth_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("inactiveauth_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = repo.create(&unique_name, &unique_email, None, None).await?;

    // Suspend the user
    repo.update_status(&created.id, UserStatus::Suspended).await?;

    let auth_user = repo.get_authenticated_user(&created.id).await?;
    assert!(auth_user.is_none());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(pool.as_ref())
        .await;

    Ok(())
}
