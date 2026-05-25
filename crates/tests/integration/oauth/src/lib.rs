//! Integration tests for systemprompt-oauth crate
//!
//! These tests require a running PostgreSQL database.
//! Set DATABASE_URL environment variable before running.

#[cfg(test)]
mod bridge_session_tests;

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

use std::env;
use std::sync::{Arc, Once};
use systemprompt_config::SecretsBootstrap;
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::UserId;
use systemprompt_users::UserRepository;
use uuid::Uuid;

pub async fn setup_test_db() -> DbPool {
    ensure_test_secrets_bootstrap();
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL environment variable required");

    let db = Database::new_postgres(&database_url)
        .await
        .expect("Failed to connect to test database");

    let db = Arc::new(db);
    seed_fixture_user(&db).await;
    db
}

async fn seed_fixture_user(db: &DbPool) {
    let pool = db.pool_arc().expect("read pool");
    sqlx::query(
        "INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(systemprompt_test_fixtures::fixture_user_id().as_str())
    .bind("test-user@example.invalid")
    .execute(pool.as_ref())
    .await
    .expect("seed fixture user");
}

fn ensure_test_secrets_bootstrap() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Tests set process env before any threads run; using the subprocess
        // bootstrap path keeps SecretsBootstrap consistent with how server
        // processes load deployment secrets in air-gapped/container modes.
        // SAFETY: single-threaded test init.
        unsafe {
            env::set_var("SYSTEMPROMPT_SUBPROCESS", "1");
            if env::var("OAUTH_AT_REST_PEPPER").is_err() {
                env::set_var(
                    "OAUTH_AT_REST_PEPPER",
                    "test_oauth_at_rest_pepper_for_integration_tests_zzz",
                );
            }
            if env::var("MANIFEST_SIGNING_SECRET_SEED").is_err() {
                env::set_var(
                    "MANIFEST_SIGNING_SECRET_SEED",
                    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                );
            }
        }
        SecretsBootstrap::try_init().expect("SecretsBootstrap::try_init should succeed in tests");
    });
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
