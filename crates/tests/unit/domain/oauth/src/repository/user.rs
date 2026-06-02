// DB-backed OAuth user lookup tests.

use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

async fn repo() -> Option<(OAuthRepository, systemprompt_database::DbPool)> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    Some((repo, pool))
}

#[tokio::test]
async fn find_user_by_email_round_trips() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    let user_id = unique_user_id("ou");
    let email = format!("{}@ou.invalid", user_id.as_str());
    seed_user_row(&pool, &user_id, &email).await.expect("seed");

    let found = repo
        .find_user_by_email(&email)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.id, user_id);
    assert_eq!(found.email, email);
    assert!(found.roles.contains(&"user".to_owned()));
}

#[tokio::test]
async fn find_user_by_email_missing_returns_none() {
    let Some((repo, _pool)) = repo().await else {
        return;
    };
    assert!(
        repo.find_user_by_email(&format!("missing-{}@ou.invalid", Uuid::new_v4()))
            .await
            .expect("find")
            .is_none()
    );
}

#[tokio::test]
async fn get_authenticated_user_resolves_permissions() {
    let Some((repo, pool)) = repo().await else {
        return;
    };
    // get_authenticated_user parses the user id as a UUID, so the row id must
    // be a bare UUID rather than a prefixed test id.
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let email = format!("{}@ou.invalid", user_id.as_str());
    seed_user_row(&pool, &user_id, &email).await.expect("seed");

    let authed = repo
        .get_authenticated_user(&user_id)
        .await
        .expect("get authenticated user");
    assert_eq!(authed.email, email);
}

#[tokio::test]
async fn get_authenticated_user_missing_errors() {
    let Some((repo, _pool)) = repo().await else {
        return;
    };
    let user_id = UserId::new(Uuid::new_v4().to_string());
    assert!(repo.get_authenticated_user(&user_id).await.is_err());
}
