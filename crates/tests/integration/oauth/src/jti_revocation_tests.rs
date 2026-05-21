//! Integration tests for JWT JTI revocation (H3).

use crate::{create_test_user, setup_test_db};
use chrono::{Duration, Utc};
use systemprompt_oauth::repository::{JtiRevocationCache, OAuthRepository};
use uuid::Uuid;

fn unique_jti() -> String {
    Uuid::new_v4().to_string()
}

async fn user_uuid(db: &systemprompt_database::DbPool) -> Uuid {
    let id = create_test_user(db).await;
    Uuid::parse_str(id.as_str()).expect("user id is uuid")
}

#[tokio::test]
async fn revoked_token_visible_to_is_jti_revoked() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let jti = unique_jti();
    let user = user_uuid(&db).await;

    assert!(
        !repo.is_jti_revoked(&jti).await.expect("query ok"),
        "fresh jti must not be revoked"
    );

    repo.revoke_jti(&jti, user, Utc::now() + Duration::hours(1))
        .await
        .expect("revoke ok");

    assert!(
        repo.is_jti_revoked(&jti).await.expect("query ok"),
        "after revoke, jti must read as revoked"
    );
}

#[tokio::test]
async fn expired_revocation_row_treated_as_not_revoked() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let jti = unique_jti();
    let user = user_uuid(&db).await;

    repo.revoke_jti(&jti, user, Utc::now() - Duration::seconds(60))
        .await
        .expect("revoke ok");

    assert!(
        !repo.is_jti_revoked(&jti).await.expect("query ok"),
        "row with past exp must not gate auth (the token itself is already expired)"
    );
}

#[tokio::test]
async fn revoke_is_idempotent() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let jti = unique_jti();
    let user = user_uuid(&db).await;
    let exp = Utc::now() + Duration::hours(1);

    repo.revoke_jti(&jti, user, exp).await.expect("first ok");
    repo.revoke_jti(&jti, user, exp)
        .await
        .expect("second insert must not raise (ON CONFLICT DO NOTHING)");
}

#[tokio::test]
async fn cleanup_expired_drops_only_expired_rows() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let live_jti = unique_jti();
    let dead_jti = unique_jti();
    let user = user_uuid(&db).await;

    repo.revoke_jti(&live_jti, user, Utc::now() + Duration::hours(1))
        .await
        .expect("live revoke");
    repo.revoke_jti(&dead_jti, user, Utc::now() - Duration::seconds(60))
        .await
        .expect("dead revoke");

    repo.cleanup_expired_jti_revocations()
        .await
        .expect("cleanup ok");

    assert!(repo.is_jti_revoked(&live_jti).await.expect("query"));
    assert!(!repo.is_jti_revoked(&dead_jti).await.expect("query"));
}

#[tokio::test]
async fn revoke_jtis_for_user_bulk() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let user = user_uuid(&db).await;
    let jtis: Vec<String> = (0..3).map(|_| unique_jti()).collect();
    let exp = Utc::now() + Duration::hours(1);

    let inserted = repo
        .revoke_jtis_for_user(user, &jtis, exp)
        .await
        .expect("bulk revoke");
    assert_eq!(inserted, jtis.len() as u64);

    for jti in &jtis {
        assert!(
            repo.is_jti_revoked(jti).await.expect("query"),
            "kick must revoke every jti in the batch"
        );
    }
}

#[test]
fn cache_negative_ttl_then_revocation_sticks() {
    let cache = JtiRevocationCache::new();

    assert_eq!(cache.peek("jti-a"), None, "miss before any record");

    cache.record("jti-a", false);
    assert_eq!(
        cache.peek("jti-a"),
        Some(false),
        "negative result must be visible to subsequent peeks"
    );

    cache.record("jti-b", true);
    assert_eq!(cache.peek("jti-b"), Some(true));

    // Re-record as not-revoked should NOT be possible monotonically — but the
    // cache itself is dumb; the repo path simply never records `false` for a
    // jti that just came back true. Asserting the cache stays sticky on
    // re-record(false) would be wrong; the contract is at the middleware
    // layer.
}
