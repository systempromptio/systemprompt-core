//! Integration tests for UserAdminService.
//!
//! Tests cover:
//! - find_user by ID, email, and name
//! - promote_to_admin
//! - demote_from_admin

use anyhow::Result;
use systemprompt_database::Database;
use systemprompt_users::{DemoteResult, PromoteResult, UserAdminService, UserService};

async fn get_db() -> Option<Database> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    Database::new_postgres(&database_url).await.ok()
}

// ============================================================================
// UserAdminService::find_user Tests
// ============================================================================

#[tokio::test]
async fn admin_find_user_by_email() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let unique_email = format!("admin_find_email_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("adminfind_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = user_service.create(&unique_name, &unique_email, None, None).await?;

    let found = admin_service.find_user(&unique_email).await?;
    assert!(found.is_some());
    assert_eq!(found.as_ref().map(|u| &u.email), Some(&unique_email));

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn admin_find_user_by_name() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let unique_email = format!("admin_find_name_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("adminfindname_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = user_service.create(&unique_name, &unique_email, None, None).await?;

    let found = admin_service.find_user(&unique_name).await?;
    assert!(found.is_some());
    assert_eq!(found.as_ref().map(|u| &u.name), Some(&unique_name));

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn admin_find_user_by_uuid() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let unique_email = format!("admin_find_uuid_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("adminfinduuid_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = user_service.create(&unique_name, &unique_email, None, None).await?;

    let found = admin_service.find_user(created.id.as_str()).await?;
    assert!(found.is_some());
    assert_eq!(found.as_ref().map(|u| u.id.to_string()), Some(created.id.to_string()));

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn admin_find_user_returns_none_for_nonexistent() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service);

    let found = admin_service.find_user("nonexistent_user_identifier").await?;
    assert!(found.is_none());

    Ok(())
}

// ============================================================================
// UserAdminService::promote_to_admin Tests
// ============================================================================

#[tokio::test]
async fn admin_promote_user_to_admin() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let unique_email = format!("admin_promote_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("adminpromote_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = user_service.create(&unique_name, &unique_email, None, None).await?;

    let result = admin_service.promote_to_admin(&unique_email).await?;

    match result {
        PromoteResult::Promoted(user, roles) => {
            assert!(roles.contains(&"admin".to_string()));
            assert!(roles.contains(&"user".to_string()));
            assert!(user.roles.contains(&"admin".to_string()));
        }
        _ => panic!("Expected Promoted result"),
    }

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn admin_promote_already_admin_returns_already_admin() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let unique_email = format!("admin_already_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("adminalready_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = user_service.create(&unique_name, &unique_email, None, None).await?;

    user_service.assign_roles(&created.id, &["admin".to_string(), "user".to_string()]).await?;

    let result = admin_service.promote_to_admin(&unique_email).await?;

    match result {
        PromoteResult::AlreadyAdmin(user) => {
            assert!(user.roles.contains(&"admin".to_string()));
        }
        _ => panic!("Expected AlreadyAdmin result"),
    }

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn admin_promote_nonexistent_returns_not_found() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service);

    let result = admin_service.promote_to_admin("nonexistent@example.com").await?;

    assert!(matches!(result, PromoteResult::UserNotFound));

    Ok(())
}

// ============================================================================
// UserAdminService::demote_from_admin Tests
// ============================================================================

#[tokio::test]
async fn admin_demote_user_from_admin() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let unique_email = format!("admin_demote_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("admindemote_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = user_service.create(&unique_name, &unique_email, None, None).await?;

    user_service.assign_roles(&created.id, &["admin".to_string(), "user".to_string()]).await?;

    let result = admin_service.demote_from_admin(&unique_email).await?;

    match result {
        DemoteResult::Demoted(user, roles) => {
            assert!(!roles.contains(&"admin".to_string()));
            assert!(roles.contains(&"user".to_string()));
            assert!(!user.roles.contains(&"admin".to_string()));
        }
        _ => panic!("Expected Demoted result"),
    }

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn admin_demote_non_admin_returns_not_admin() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let unique_email = format!("admin_nonadmin_{}@example.com", uuid::Uuid::new_v4());
    let unique_name = format!("adminnonadmin_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let created = user_service.create(&unique_name, &unique_email, None, None).await?;

    let result = admin_service.demote_from_admin(&unique_email).await?;

    match result {
        DemoteResult::NotAdmin(user) => {
            assert!(!user.roles.contains(&"admin".to_string()));
        }
        _ => panic!("Expected NotAdmin result"),
    }

    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(created.id.as_str())
        .execute(db.pool_arc()?.as_ref())
        .await;

    Ok(())
}

#[tokio::test]
async fn admin_demote_nonexistent_returns_not_found() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    let db_pool = db.as_pool()?;
    let user_service = UserService::new(&db_pool)?;
    let admin_service = UserAdminService::new(user_service);

    let result = admin_service.demote_from_admin("nonexistent@example.com").await?;

    assert!(matches!(result, DemoteResult::UserNotFound));

    Ok(())
}
