//! DB-backed tests for the user-scoped session read queries
//! (`find_by_fingerprint`, `list_active_by_user`). These require a real
//! `users` row because they filter on `user_id`, so a user is seeded first.

use chrono::{Duration, Utc};
use systemprompt_analytics::{CreateSessionParams, SessionRepository};
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row,
};
use uuid::Uuid;

use super::session_support::delete_session;

fn params_for_user<'a>(
    session_id: &'a SessionId,
    user_id: &'a UserId,
    fingerprint: &'a str,
) -> CreateSessionParams<'a> {
    CreateSessionParams {
        session_id,
        user_id: Some(user_id),
        session_source: SessionSource::Web,
        fingerprint_hash: Some(fingerprint),
        ip_address: None,
        user_agent: None,
        device_type: None,
        browser: None,
        os: None,
        country: None,
        region: None,
        city: None,
        preferred_locale: None,
        referrer_source: None,
        referrer_url: None,
        landing_page: None,
        entry_url: None,
        utm_source: None,
        utm_medium: None,
        utm_campaign: None,
        utm_content: None,
        utm_term: None,
        is_bot: false,
        is_ai_crawler: false,
        expires_at: Utc::now() + Duration::hours(1),
    }
}

#[tokio::test]
async fn find_by_fingerprint_returns_active_session_for_user() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let user = UserId::new(format!("user-{}", Uuid::new_v4()));
    seed_user_row(&pool, &user, &format!("{}@t.test", user.as_str()))
        .await
        .expect("seed user");

    let fp = format!("fp-{}", Uuid::new_v4());
    let sid = SessionId::new(format!("sess-{}", Uuid::new_v4()));
    repo.create_session(&params_for_user(&sid, &user, &fp))
        .await
        .expect("create");

    let found = repo
        .find_by_fingerprint(&fp, &user)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.session_id.as_str(), sid.as_str());
    assert_eq!(
        found.user_id.as_ref().map(UserId::as_str),
        Some(user.as_str())
    );

    // A different user with the same fingerprint sees nothing.
    let other = UserId::new(format!("user-{}", Uuid::new_v4()));
    seed_user_row(&pool, &other, &format!("{}@t.test", other.as_str()))
        .await
        .expect("seed other");
    assert!(
        repo.find_by_fingerprint(&fp, &other)
            .await
            .expect("find other")
            .is_none()
    );

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn list_active_by_user_returns_only_open_sessions() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = SessionRepository::new(&pool).expect("repo");

    let user = UserId::new(format!("user-{}", Uuid::new_v4()));
    seed_user_row(&pool, &user, &format!("{}@t.test", user.as_str()))
        .await
        .expect("seed user");

    let open = SessionId::new(format!("sess-{}", Uuid::new_v4()));
    let ended = SessionId::new(format!("sess-{}", Uuid::new_v4()));
    repo.create_session(&params_for_user(
        &open,
        &user,
        &format!("fp-{}", Uuid::new_v4()),
    ))
    .await
    .expect("create open");
    repo.create_session(&params_for_user(
        &ended,
        &user,
        &format!("fp-{}", Uuid::new_v4()),
    ))
    .await
    .expect("create ended");
    repo.end_session(&ended).await.expect("end");

    let active = repo.list_active_by_user(&user).await.expect("list");
    let ids: Vec<&str> = active.iter().map(|s| s.session_id.as_str()).collect();
    assert!(ids.contains(&open.as_str()), "open session must be listed");
    assert!(
        !ids.contains(&ended.as_str()),
        "ended session must be excluded"
    );

    delete_session(&pool, &open).await;
    delete_session(&pool, &ended).await;
}
