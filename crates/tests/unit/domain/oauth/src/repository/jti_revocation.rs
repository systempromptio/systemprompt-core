// JtiRevocationCache pure-logic tests + DB-backed jti revocation round-trips.

use chrono::{Duration, Utc};
use systemprompt_oauth::repository::{JtiRevocationCache, OAuthRepository};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

#[test]
fn cache_miss_returns_none() {
    let cache = JtiRevocationCache::new();
    assert_eq!(cache.peek("never-seen"), None);
}

#[test]
fn cache_records_negative_then_positive() {
    let cache = JtiRevocationCache::with_capacity(16);
    cache.record("jti-a", false);
    assert_eq!(cache.peek("jti-a"), Some(false));
    cache.record("jti-a", true);
    assert_eq!(cache.peek("jti-a"), Some(true));
}

#[test]
fn cache_revoked_is_sticky() {
    let cache = JtiRevocationCache::default();
    cache.record("jti-b", true);
    assert_eq!(cache.peek("jti-b"), Some(true));
    assert_eq!(cache.peek("jti-b"), Some(true));
}

#[test]
fn cache_capacity_zero_clamps_to_one() {
    let cache = JtiRevocationCache::with_capacity(0);
    cache.record("only", true);
    assert_eq!(cache.peek("only"), Some(true));
}

#[tokio::test]
async fn revoke_jti_then_is_revoked() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");

    let uid = Uuid::new_v4();
    let jti = format!("jti-{}", Uuid::new_v4());
    let exp = Utc::now() + Duration::hours(1);

    assert!(!repo.is_jti_revoked(&jti).await.expect("check before"));
    repo.revoke_jti(&jti, uid, exp).await.expect("revoke");
    assert!(repo.is_jti_revoked(&jti).await.expect("check after"));
}

#[tokio::test]
async fn revoke_jti_is_idempotent_on_conflict() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");

    let uid = Uuid::new_v4();
    let jti = format!("jti-{}", Uuid::new_v4());
    let exp = Utc::now() + Duration::hours(1);
    repo.revoke_jti(&jti, uid, exp).await.expect("first");
    repo.revoke_jti(&jti, uid, exp).await.expect("second");
    assert!(repo.is_jti_revoked(&jti).await.expect("check"));
}

#[tokio::test]
async fn expired_jti_not_revoked() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");

    let uid = Uuid::new_v4();
    let jti = format!("jti-{}", Uuid::new_v4());
    let exp = Utc::now() - Duration::hours(1);
    repo.revoke_jti(&jti, uid, exp).await.expect("revoke");
    // exp is in the past, so is_jti_revoked filters it out.
    assert!(!repo.is_jti_revoked(&jti).await.expect("check"));
}

#[tokio::test]
async fn revoke_jtis_for_user_batch() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");

    let uid = Uuid::new_v4();
    let jtis = vec![
        format!("jti-{}", Uuid::new_v4()),
        format!("jti-{}", Uuid::new_v4()),
        format!("jti-{}", Uuid::new_v4()),
    ];
    let exp = Utc::now() + Duration::hours(1);
    let inserted = repo
        .revoke_jtis_for_user(uid, &jtis, exp)
        .await
        .expect("batch revoke");
    assert_eq!(inserted, 3);
    for jti in &jtis {
        assert!(repo.is_jti_revoked(jti).await.expect("check"));
    }
}

#[tokio::test]
async fn cleanup_expired_jti_revocations_removes_past_rows() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");

    let uid = Uuid::new_v4();
    let jti = format!("jti-{}", Uuid::new_v4());
    let exp = Utc::now() - Duration::hours(2);
    repo.revoke_jti(&jti, uid, exp).await.expect("revoke");
    let removed = repo
        .cleanup_expired_jti_revocations()
        .await
        .expect("cleanup");
    assert!(removed >= 1);
}
