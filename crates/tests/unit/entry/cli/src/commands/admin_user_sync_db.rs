//! Database-backed tests for admin-user sync.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::sync::admin_user::{CloudUser, SyncResult, sync_admin_to_database};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use systemprompt_users::UserService;
use uuid::Uuid;

fn unique_user(prefix: &str) -> CloudUser {
    CloudUser {
        email: format!("{prefix}-{}@sync.test", Uuid::new_v4().simple()),
        name: Some("Sync Tester".to_owned()),
    }
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

#[test]
fn cloud_user_username_is_email_local_part() {
    let user = CloudUser {
        email: "person@example.com".to_owned(),
        name: None,
    };
    assert_eq!(user.username(), "person");

    let odd = CloudUser {
        email: "no-at-sign".to_owned(),
        name: None,
    };
    assert_eq!(odd.username(), "no-at-sign");
}

#[tokio::test]
async fn unreachable_database_reports_connection_failure() {
    let user = unique_user("unreachable");
    let result = sync_admin_to_database(
        &user,
        "postgres://nobody:nothing@127.0.0.1:1/absent",
        "ghost-profile",
    )
    .await;

    match result {
        SyncResult::ConnectionFailed { profile, error } => {
            assert_eq!(profile, "ghost-profile");
            assert!(!error.is_empty());
        },
        other => panic!("expected connection failure, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_user_is_created_then_reported_admin_on_resync() {
    let url = fixture_database_url().unwrap();
    let user = unique_user("created");

    match sync_admin_to_database(&user, &url, "local").await {
        SyncResult::Created { email, profile } => {
            assert_eq!(email, user.email);
            assert_eq!(profile, "local");
        },
        other => panic!("expected creation, got {other:?}"),
    }

    match sync_admin_to_database(&user, &url, "local").await {
        SyncResult::AlreadyAdmin { email, .. } => assert_eq!(email, user.email),
        other => panic!("expected already-admin, got {other:?}"),
    }
}

#[tokio::test]
async fn existing_regular_user_is_promoted() {
    let url = fixture_database_url().unwrap();
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();

    let user = unique_user("promoted");
    service
        .create(&user.username(), &user.email, None, None)
        .await
        .unwrap();

    match sync_admin_to_database(&user, &url, "local").await {
        SyncResult::Promoted { email, profile } => {
            assert_eq!(email, user.email);
            assert_eq!(profile, "local");
        },
        other => panic!("expected promotion, got {other:?}"),
    }
}
