//! `JtiRevocationChecker` — the last stateful gate before a request is
//! admitted.
//!
//! Signature validation proves a token was issued; only this answers whether
//! it has since been withdrawn. It sits behind an LRU whose two directions are
//! deliberately asymmetric — a positive result is sticky, a negative one is
//! held for 60 seconds — so the suite drives the checker rather than the
//! repository, because the asymmetry is the behaviour worth pinning and it is
//! invisible from the query alone.

use chrono::{Duration, Utc};
use systemprompt_api::services::middleware::JtiRevocationChecker;
use systemprompt_database::DbPool;
use systemprompt_models::execution::context::ContextExtractionError;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().expect("DATABASE_URL"))
        .await
        .expect("the revocation tests need a reachable test database")
}

fn checker(pool: &DbPool) -> JtiRevocationChecker {
    JtiRevocationChecker::from_repository(OAuthRepository::new(pool).expect("oauth repository"))
}

fn jti() -> String {
    format!("jti-revocation-{}", Uuid::new_v4().simple())
}

async fn revoke(pool: &DbPool, jti: &str, minutes_from_now: i64) {
    let p = pool.pool_arc().expect("write pool");
    sqlx::query(
        "INSERT INTO oauth_jti_revocations (jti, user_id, exp) VALUES ($1, $2, $3) \
         ON CONFLICT (jti) DO UPDATE SET exp = EXCLUDED.exp",
    )
    .bind(jti)
    .bind(Uuid::new_v4())
    .bind(Utc::now() + Duration::minutes(minutes_from_now))
    .execute(&*p)
    .await
    .expect("record the revocation");
}

async fn forget(pool: &DbPool, jti: &str) {
    let p = pool.pool_arc().expect("write pool");
    sqlx::query("DELETE FROM oauth_jti_revocations WHERE jti = $1")
        .bind(jti)
        .execute(&*p)
        .await
        .expect("drop the revocation");
}

fn is_revoked_error(result: &Result<(), ContextExtractionError>) -> bool {
    matches!(result, Err(ContextExtractionError::Revoked))
}

#[tokio::test]
async fn a_token_that_was_never_revoked_is_admitted() {
    let pool = pool().await;
    checker(&pool)
        .ensure_not_revoked(&jti())
        .await
        .expect("a jti with no revocation row must pass the gate");
}

#[tokio::test]
async fn a_revoked_token_is_refused() {
    let pool = pool().await;
    let jti = jti();
    revoke(&pool, &jti, 60).await;

    let result = checker(&pool).ensure_not_revoked(&jti).await;

    assert!(
        is_revoked_error(&result),
        "a live revocation row must reject the token, got {result:?}"
    );
}

// Why: the row carries the token's own expiry, and the lookup filters on
// `exp > now()`. Without that filter a revocation would outlive the token it
// describes, so a jti reissued after cleanup — or simply a stale row cleanup
// has not reached yet — would keep rejecting a legitimate token. Dropping the
// filter is the mutation this test exists to catch.
#[tokio::test]
async fn a_revocation_that_has_itself_expired_no_longer_refuses() {
    let pool = pool().await;
    let jti = jti();
    revoke(&pool, &jti, -60).await;

    checker(&pool)
        .ensure_not_revoked(&jti)
        .await
        .expect("a revocation past its own expiry is spent and must not reject");
}

// Why: a token with no `jti` claim cannot be named in a revocation row, so
// there is nothing to look up. It is admitted deliberately — the gate above
// this one decides whether such a token is acceptable at all.
#[tokio::test]
async fn a_token_with_no_jti_claim_is_not_checked() {
    let pool = pool().await;
    checker(&pool)
        .ensure_not_revoked("")
        .await
        .expect("an empty jti has nothing to look up");
}

// Why: the positive half of the cache is sticky on purpose. Once a token has
// been seen revoked, no later database state may bring it back — otherwise a
// cleanup job deleting spent rows would silently re-admit tokens that are
// still in an attacker's hands.
#[tokio::test]
async fn a_token_seen_revoked_stays_refused_after_the_row_is_deleted() {
    let pool = pool().await;
    let jti = jti();
    revoke(&pool, &jti, 60).await;

    let checker = checker(&pool);
    assert!(
        is_revoked_error(&checker.ensure_not_revoked(&jti).await),
        "precondition: the first check must observe the revocation"
    );

    forget(&pool, &jti).await;

    let result = checker.ensure_not_revoked(&jti).await;
    assert!(
        is_revoked_error(&result),
        "a revoked jti must never become un-revoked, got {result:?}"
    );
}

// Why: the negative half is a 60-second staleness window, and it is a real
// one — a token revoked immediately after being admitted keeps working until
// the entry ages out. That is a deliberate trade for a map lookup on the hot
// path, but it is not obvious from reading the checker, and widening the TTL
// widens the window. This pins it so the cost is chosen rather than inherited.
//
// The second half proves the admission is the cache and not the database: a
// checker with a cold cache sees the revocation the warm one missed.
#[tokio::test]
async fn a_token_admitted_before_revocation_keeps_working_until_the_cache_ages_out() {
    let pool = pool().await;
    let jti = jti();

    let warm = checker(&pool);
    warm.ensure_not_revoked(&jti)
        .await
        .expect("precondition: the token is admitted before it is revoked");

    revoke(&pool, &jti, 60).await;

    warm.ensure_not_revoked(&jti)
        .await
        .expect("the cached negative result holds for its TTL");

    let result = checker(&pool).ensure_not_revoked(&jti).await;
    assert!(
        is_revoked_error(&result),
        "a cold checker must see the revocation, proving the admission above \
         was the cache rather than a missing row: {result:?}"
    );
}
