//! Integration tests for systemprompt-oauth crate
//!
//! These tests require a running PostgreSQL database.
//! Set DATABASE_URL environment variable before running.

#[cfg(test)]
mod client_tests;

#[cfg(test)]
mod token_tests;

#[cfg(test)]
mod webauthn_tests;

use std::env;
use std::sync::Arc;
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::UserId;
use systemprompt_users::UserRepository;
use uuid::Uuid;

pub async fn setup_test_db() -> DbPool {
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL environment variable required");

    let db = Database::new_postgres(&database_url)
        .await
        .expect("Failed to connect to test database");

    Arc::new(db)
}

pub async fn create_test_user(db: &DbPool) -> UserId {
    let repo = UserRepository::new(db).expect("Failed to create user repository");
    let unique_id = Uuid::new_v4();
    let name = format!("test_user_{}", unique_id);
    let email = format!("test_{}@example.com", unique_id);

    let user = repo
        .create(&name, &email, Some("Test User"), Some("Test"))
        .await
        .expect("Failed to create test user");

    UserId::new(user.id.clone())
}

pub async fn cleanup_test_user(db: &DbPool, user_id: &UserId) {
    let repo = UserRepository::new(db).expect("Failed to create user repository");
    let _ = repo.delete(user_id).await;
}
