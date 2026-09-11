//! `admin session login` admin-user resolution: who is accepted as the admin,
//! what a local profile refuses, and what a cloud profile is allowed to create.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use systemprompt_cli::admin::session::login_helpers::fetch_admin_user;
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use systemprompt_users::{User, UserRepository, UserRole, UserService};
use uuid::Uuid;

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn service(pool: &DbPool) -> UserService {
    UserService::new(Arc::new(UserRepository::new(pool).unwrap()))
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn seed(pool: &DbPool, name: &str, admin: bool) -> User {
    let service = service(pool);
    let user = service
        .create(name, &format!("{name}@login.invalid"), None, None)
        .await
        .unwrap();
    if admin {
        return service
            .assign_roles(&user.id, &[UserRole::Admin.as_str().to_owned()])
            .await
            .unwrap();
    }
    user
}

#[tokio::test]
async fn an_existing_admin_is_returned_without_being_recreated() {
    let pool = pool().await;
    let name = unique("login-admin");
    let seeded = seed(&pool, &name, true).await;

    let found = fetch_admin_user(&pool, &name, false, None)
        .await
        .expect("an existing admin resolves");

    assert_eq!(
        found.id.as_str(),
        seeded.id.as_str(),
        "login must reuse the seeded admin row, not mint a second user"
    );
    assert!(
        found.is_admin(),
        "the resolved user must hold the admin role"
    );
}

#[tokio::test]
async fn a_user_without_the_admin_role_is_refused_rather_than_promoted() {
    let pool = pool().await;
    let name = unique("login-plain");
    seed(&pool, &name, false).await;

    let err = fetch_admin_user(&pool, &name, true, Some("promoted@login.invalid"))
        .await
        .expect_err("a non-admin user must not be accepted as the admin");

    let message = format!("{err:#}");
    assert!(
        message.contains("is not an admin"),
        "the refusal must say the user is not an admin, got: {message}"
    );

    let still = service(&pool)
        .find_by_name(&name)
        .await
        .unwrap()
        .expect("the user still exists");
    assert!(
        !still.is_admin(),
        "a refused login must leave the user's roles untouched"
    );
}

#[tokio::test]
async fn a_missing_admin_on_a_local_profile_is_told_to_bootstrap() {
    let pool = pool().await;
    let name = unique("login-absent");

    let err = fetch_admin_user(&pool, &name, false, None)
        .await
        .expect_err("a local profile must not create the admin user");

    let message = format!("{err:#}");
    assert!(
        message.contains("admin bootstrap"),
        "the failure must name the bootstrap command, got: {message}"
    );
    assert!(
        service(&pool).find_by_name(&name).await.unwrap().is_none(),
        "a local profile must not have created the user"
    );
}

#[tokio::test]
async fn a_missing_admin_on_a_cloud_profile_is_created_with_the_admin_role() {
    let pool = pool().await;
    let name = unique("login-cloud");
    let email = format!("{name}@cloud.invalid");

    let created = fetch_admin_user(&pool, &name, true, Some(&email))
        .await
        .expect("a cloud profile provisions its admin user");

    assert_eq!(created.name, name);
    assert_eq!(created.email, email);
    assert!(
        created.is_admin(),
        "a provisioned cloud admin must be given the admin role, not just a user row"
    );

    let persisted = service(&pool)
        .find_by_name(&name)
        .await
        .unwrap()
        .expect("the created admin is persisted");
    assert!(
        persisted.is_admin(),
        "the admin role must be persisted, not only returned"
    );
}
