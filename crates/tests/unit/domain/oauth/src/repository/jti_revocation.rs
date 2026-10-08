// JtiRevocationCache pure-logic tests + DB-backed jti revocation round-trips.

use chrono::{Duration, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AccessTokenId, UserId};
use systemprompt_oauth::repository::{JtiRevocationCache, OAuthRepository, OauthCleanupRepository};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use uuid::Uuid;

fn jti(label: &str) -> AccessTokenId {
    AccessTokenId::new(label)
}

fn fresh_jti() -> AccessTokenId {
    AccessTokenId::new(format!("jti-{}", Uuid::new_v4()))
}

async fn seeded_user(pool: &DbPool) -> UserId {
    let uid = unique_user_id("jti-owner");
    seed_user_row(pool, &uid, &format!("{}@jti.invalid", uid.as_str()))
        .await
        .expect("seed user");
    uid
}

#[test]
fn cache_miss_returns_none() {
    let cache = JtiRevocationCache::new();
    assert_eq!(cache.peek(&jti("never-seen")), None);
}

#[test]
fn cache_records_negative_then_positive() {
    let cache = JtiRevocationCache::with_capacity(16);
    cache.record(&jti("jti-a"), false);
    assert_eq!(cache.peek(&jti("jti-a")), Some(false));
    cache.record(&jti("jti-a"), true);
    assert_eq!(cache.peek(&jti("jti-a")), Some(true));
}

#[test]
fn cache_revoked_is_sticky() {
    let cache = JtiRevocationCache::default();
    cache.record(&jti("jti-b"), true);
    assert_eq!(cache.peek(&jti("jti-b")), Some(true));
    assert_eq!(cache.peek(&jti("jti-b")), Some(true));
}

#[test]
fn cache_capacity_zero_clamps_to_one() {
    let cache = JtiRevocationCache::with_capacity(0);
    cache.record(&jti("only"), true);
    assert_eq!(cache.peek(&jti("only")), Some(true));
}

#[tokio::test]
async fn revoke_jti_then_is_revoked() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);

    let uid = seeded_user(&pool).await;
    let jti = fresh_jti();
    let exp = Utc::now() + Duration::hours(1);

    assert!(!repo.is_jti_revoked(&jti).await.expect("check before"));
    repo.revoke_jti(&jti, &uid, exp).await.expect("revoke");
    assert!(repo.is_jti_revoked(&jti).await.expect("check after"));
}

#[tokio::test]
async fn revoke_jti_is_idempotent_on_conflict() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);

    let uid = seeded_user(&pool).await;
    let jti = fresh_jti();
    let exp = Utc::now() + Duration::hours(1);
    repo.revoke_jti(&jti, &uid, exp).await.expect("first");
    repo.revoke_jti(&jti, &uid, exp).await.expect("second");
    assert!(repo.is_jti_revoked(&jti).await.expect("check"));
}

#[tokio::test]
async fn expired_jti_not_revoked() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);

    let uid = seeded_user(&pool).await;
    let jti = fresh_jti();
    let exp = Utc::now() - Duration::hours(1);
    repo.revoke_jti(&jti, &uid, exp).await.expect("revoke");
    // exp is in the past, so is_jti_revoked filters it out.
    assert!(!repo.is_jti_revoked(&jti).await.expect("check"));
}

#[tokio::test]
async fn revoke_jtis_for_user_batch() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);

    let uid = seeded_user(&pool).await;
    let jtis = vec![fresh_jti(), fresh_jti(), fresh_jti()];
    let exp = Utc::now() + Duration::hours(1);
    let inserted = repo
        .revoke_jtis_for_user(&uid, &jtis, exp)
        .await
        .expect("batch revoke");
    assert_eq!(inserted, 3);
    for jti in &jtis {
        assert!(repo.is_jti_revoked(jti).await.expect("check"));
    }
}

#[tokio::test]
async fn cleanup_expired_jti_revocations_removes_past_rows() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);

    let uid = seeded_user(&pool).await;
    let jti = fresh_jti();
    let exp = Utc::now() - Duration::hours(2);
    repo.revoke_jti(&jti, &uid, exp).await.expect("revoke");
    let removed = OauthCleanupRepository::new(&pool)
        .delete_expired_jti_revocations()
        .await
        .expect("cleanup");
    assert!(removed >= 1);
}

#[test]
fn cache_with_zero_capacity_is_clamped_and_usable() {
    let cache = JtiRevocationCache::with_capacity(0);
    cache.record(&jti("jti-a"), true);
    assert_eq!(cache.peek(&jti("jti-a")), Some(true));

    cache.record(&jti("jti-b"), false);
    assert_eq!(cache.peek(&jti("jti-b")), Some(false));
    assert_eq!(
        cache.peek(&jti("jti-a")),
        None,
        "clamped single-slot cache evicts the older entry"
    );
    assert!(format!("{cache:?}").contains("JtiRevocationCache"));
}

#[tokio::test]
async fn non_uuid_user_can_revoke_a_jti() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);

    let uid = seeded_user(&pool).await;
    assert!(
        Uuid::parse_str(uid.as_str()).is_err(),
        "precondition: the owner id is not a UUID"
    );
    let jti = fresh_jti();
    repo.revoke_jti(&jti, &uid, Utc::now() + Duration::hours(1))
        .await
        .expect("a non-UUID user id is a valid revocation owner");
    assert!(repo.is_jti_revoked(&jti).await.expect("check"));
}
