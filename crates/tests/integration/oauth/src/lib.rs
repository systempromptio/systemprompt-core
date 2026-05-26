//! Integration tests for systemprompt-oauth crate
//!
//! These tests require a running PostgreSQL database.
//! Set DATABASE_URL environment variable before running.

#[cfg(test)]
mod bridge_session_tests;

#[cfg(test)]
mod bridge_extra_tests;

#[cfg(test)]
mod bridge_session_repo_tests;

#[cfg(test)]
mod setup_token_tests;

#[cfg(test)]
mod client_tests;

#[cfg(test)]
mod jti_revocation_tests;

#[cfg(test)]
mod state_binding_tests;

#[cfg(test)]
mod token_tests;

#[cfg(test)]
mod token_concurrency_tests;

#[cfg(test)]
mod webauthn_tests;

use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    ensure_test_secrets_bootstrap, fixture_database_url, fixture_db_pool,
};
use systemprompt_users::UserRepository;
use uuid::Uuid;

pub async fn setup_test_db() -> DbPool {
    ensure_test_secrets_bootstrap();
    let url = fixture_database_url().expect("DATABASE_URL");
    let db = fixture_db_pool(&url).await.expect("connect test database");
    seed_fixture_user(&db).await;
    db
}

async fn seed_fixture_user(db: &DbPool) {
    let pool = db.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(systemprompt_test_fixtures::fixture_user_id().as_str())
        .bind("test-user@example.invalid")
        .execute(pool.as_ref())
        .await
        .expect("seed fixture user");
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

    user.id.clone()
}

pub async fn cleanup_test_user(db: &DbPool, user_id: &UserId) {
    let repo = UserRepository::new(db).expect("Failed to create user repository");
    let _ = repo.delete(user_id).await;
}
