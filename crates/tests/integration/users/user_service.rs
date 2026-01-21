//! Integration tests for UserService.
//!
//! Tests cover:
//! - UserService creation
//! - Delegation to UserRepository
//! - Business logic methods

use anyhow::Result;
use systemprompt_database::Database;
use systemprompt_users::{UserRole, UserService, UserStatus};

async fn get_db() -> Option<Database> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    Database::new_postgres(&database_url).await.ok()
}

// ============================================================================
// UserService::new Tests
// ============================================================================

#[tokio::test]
async fn service_creation_from_db_pool() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    // Verify the service is functional
    let _ = service.count().await?;

    Ok(())
}

// ============================================================================
// UserService CRUD Operations
// ============================================================================

#[tokio::test]
async fn service_create_and_find_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_create_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svccreate_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let created = service
        .create(&unique_name, &unique_email, Some("Service Test"), None)
        .await?;

    assert_eq!(created.name, unique_name);
    assert_eq!(created.email, unique_email);

    // Find by ID
    let found = service.find_by_id(&created.id).await?;
    assert!(found.is_some());

    // Find by email
    let found_email = service.find_by_email(&unique_email).await?;
    assert!(found_email.is_some());

    // Find by name
    let found_name = service.find_by_name(&unique_name).await?;
    assert!(found_name.is_some());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_create_anonymous_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let fingerprint = format!("svc_anon_{}", uuid::Uuid::new_v4());
    let created = service.create_anonymous(&fingerprint).await?;

    assert!(created.name.starts_with("anonymous_"));
    assert!(created.roles.contains(&"anonymous".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserService List/Query Operations
// ============================================================================

#[tokio::test]
async fn service_list_users() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let users = service.list(10, 0).await?;
    assert!(users.len() <= 10);

    Ok(())
}

#[tokio::test]
async fn service_list_all_users() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let users = service.list_all().await?;
    // Just verify it runs
    let _ = users;

    Ok(())
}

#[tokio::test]
async fn service_search_users() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_search_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcsearch_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let results = service.search(&unique_name[0..10], 10).await?;
    assert!(results.iter().any(|u| u.id.to_string() == created.id.to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_count_users() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let count = service.count().await?;
    assert!(count >= 0);

    Ok(())
}

#[tokio::test]
async fn service_find_by_role() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let users = service.find_by_role(UserRole::User).await?;
    // All returned users should have the "user" role
    for user in users {
        assert!(user.roles.contains(&"user".to_string()));
    }

    Ok(())
}

// ============================================================================
// UserService Update Operations
// ============================================================================

#[tokio::test]
async fn service_update_email() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_upd_email_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcupdemail_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let new_email = format!("svc_new_{}@example.com", uuid::Uuid::new_v4());
    let updated = service.update_email(&created.id, &new_email).await?;

    assert_eq!(updated.email, new_email);

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_update_status() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_upd_status_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcupdstatus_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let updated = service.update_status(&created.id, UserStatus::Suspended).await?;
    assert_eq!(updated.status, Some("suspended".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_assign_roles() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_roles_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcroles_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let roles = vec!["admin".to_string(), "user".to_string()];
    let updated = service.assign_roles(&created.id, &roles).await?;

    assert!(updated.roles.contains(&"admin".to_string()));
    assert!(updated.roles.contains(&"user".to_string()));

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserService Delete Operations
// ============================================================================

#[tokio::test]
async fn service_delete_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_delete_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcdelete_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    service.delete(&created.id).await?;

    let found = service.find_by_id(&created.id).await?;
    assert!(found.is_none());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_delete_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let fingerprint = format!("svc_del_user_{}", uuid::Uuid::new_v4());
    let created = service.create_anonymous(&fingerprint).await?;

    service.delete(&created.id).await?;

    let found = service.find_by_email(&created.email).await?;
    assert!(found.is_none());

    Ok(())
}

#[tokio::test]
async fn service_cleanup_old_anonymous() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let deleted = service.cleanup_old_anonymous(30).await?;
    assert!(deleted >= 0);

    Ok(())
}

// ============================================================================
// UserService Special Queries
// ============================================================================

#[tokio::test]
async fn service_find_first_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let first = service.find_first_user().await?;
    // May or may not exist
    let _ = first;

    Ok(())
}

#[tokio::test]
async fn service_find_first_admin() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let admin = service.find_first_admin().await?;
    if let Some(user) = admin {
        assert!(user.roles.contains(&"admin".to_string()));
    }

    Ok(())
}

#[tokio::test]
async fn service_get_authenticated_user() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_auth_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcauth_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let auth = service.get_authenticated_user(&created.id).await?;
    assert!(auth.is_some());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_is_temporary_anonymous() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let fingerprint = format!("svc_temp_anon_{}", uuid::Uuid::new_v4());
    let created = service.create_anonymous(&fingerprint).await?;

    let is_temp = service.is_temporary_anonymous(&created.id).await?;
    // Anonymous users should be temporary
    assert!(is_temp);

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

// ============================================================================
// UserService Session Operations
// ============================================================================

#[tokio::test]
async fn service_list_sessions() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_sessions_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcsessions_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let sessions = service.list_sessions(&created.id).await?;
    // Newly created user has no sessions
    assert!(sessions.is_empty());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_list_active_sessions() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_active_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcactive_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let sessions = service.list_active_sessions(&created.id).await?;
    assert!(sessions.is_empty());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_list_recent_sessions() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_recent_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcrecent_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let sessions = service.list_recent_sessions(&created.id, 5).await?;
    assert!(sessions.is_empty());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_list_non_anonymous_with_sessions() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let users = service.list_non_anonymous_with_sessions(10).await?;
    // All returned should NOT have anonymous role
    for user in users {
        assert!(!user.roles.contains(&"anonymous".to_string()));
    }

    Ok(())
}

#[tokio::test]
async fn service_get_with_sessions() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_with_sess_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcwithsess_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let user_with_sessions = service.get_with_sessions(&created.id).await?;
    assert!(user_with_sessions.is_some());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn service_get_activity() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let service = UserService::new(&db_pool)?;

    let unique_email = format!("svc_activity_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("svcactivity_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = service.create(&unique_name, &unique_email, None, None).await?;

    let activity = service.get_activity(&created.id).await?;
    assert_eq!(activity.user_id.to_string(), created.id.to_string());

    // Cleanup
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}
