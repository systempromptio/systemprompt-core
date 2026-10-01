//! Decode failures, TTL bounds and physical challenge expiry for the
//! `WebAuthn` persistence layer.
//!
//! `consume_webauthn_challenge` filters on `expires_at`, so it reports None
//! whether or not a row was actually deleted; the cleanup test counts rows.

use sqlx::PgPool;
use std::time::Duration;
use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::{
    OAuthRepository, OauthCleanupRepository, StoreChallengeParams, WebAuthnChallengeKind,
    WebAuthnCredentialParams,
};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: OAuthRepository,
    cleanup: OauthCleanupRepository,
    write: std::sync::Arc<PgPool>,
    user_id: UserId,
}

async fn setup(prefix: &str) -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool).expect("repo");
    let cleanup = OauthCleanupRepository::new(&pool).expect("cleanup repo");
    let write = pool.write_pool_arc().expect("write pool");
    let user_id = unique_user_id(prefix);
    seed_user_row(&pool, &user_id, &format!("{}@wa.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    Ctx {
        repo,
        cleanup,
        write,
        user_id,
    }
}

async fn store_credential(ctx: &Ctx, id: &str) -> Vec<u8> {
    let credential_id = Uuid::new_v4().as_bytes().to_vec();
    ctx.repo
        .store_webauthn_credential(
            WebAuthnCredentialParams::builder(id, &ctx.user_id, &credential_id, &[7u8])
                .with_device_type("platform")
                .build(),
        )
        .await
        .expect("store");
    credential_id
}

#[tokio::test]
async fn list_rejects_a_credential_row_with_malformed_transports() {
    let ctx = setup("wa-badtr").await;
    let id = format!("cred-{}", Uuid::new_v4());
    let credential_id = store_credential(&ctx, &id).await;

    sqlx::query("UPDATE webauthn_credentials SET transports = 'usb' WHERE credential_id = $1")
        .bind(&credential_id)
        .execute(&*ctx.write)
        .await
        .expect("corrupt transports");

    assert!(
        ctx.repo
            .list_webauthn_credentials(&ctx.user_id)
            .await
            .is_err(),
        "transports that are not a JSON list must fail loudly"
    );
}

#[tokio::test]
async fn list_returns_the_newest_credential_first() {
    let ctx = setup("wa-order").await;
    let older = format!("cred-old-{}", Uuid::new_v4());
    let newer = format!("cred-new-{}", Uuid::new_v4());
    let older_credential = store_credential(&ctx, &older).await;
    store_credential(&ctx, &newer).await;

    sqlx::query(
        "UPDATE webauthn_credentials SET created_at = NOW() - INTERVAL '1 day' \
         WHERE credential_id = $1",
    )
    .bind(&older_credential)
    .execute(&*ctx.write)
    .await
    .expect("age the older credential");

    let creds = ctx
        .repo
        .list_webauthn_credentials(&ctx.user_id)
        .await
        .expect("list");
    let ids: Vec<&str> = creds.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![newer.as_str(), older.as_str()],
        "credentials must come back newest-first"
    );
}

#[tokio::test]
async fn touching_an_unknown_credential_fails_and_changes_nothing() {
    let ctx = setup("wa-noop").await;
    let id = format!("cred-{}", Uuid::new_v4());
    store_credential(&ctx, &id).await;
    let unknown = Uuid::new_v4();

    ctx.repo
        .touch_webauthn_credential(unknown.as_bytes())
        .await
        .expect_err("an update that matches no credential must be reported");

    let creds = ctx
        .repo
        .list_webauthn_credentials(&ctx.user_id)
        .await
        .expect("list");
    let found = creds.iter().find(|c| c.id == id).expect("present");
    assert!(
        found.last_used_at.is_none(),
        "an unrelated credential must be untouched"
    );
}

#[tokio::test]
async fn storing_a_challenge_with_an_unrepresentable_ttl_is_rejected() {
    let ctx = setup("wa-ttl").await;
    let challenge = format!("ttlmax-{}", Uuid::new_v4().simple());

    let err = ctx
        .repo
        .store_webauthn_challenge(StoreChallengeParams {
            challenge: &challenge,
            kind: WebAuthnChallengeKind::Registration,
            user_id: Some(&ctx.user_id),
            state: &serde_json::Value::Null,
            oauth_state: None,
            ttl: Duration::MAX,
        })
        .await
        .expect_err("a TTL beyond chrono's range must not be stored");
    assert!(
        err.to_string().contains("Challenge TTL out of range"),
        "got {err}"
    );

    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM webauthn_challenges WHERE challenge = $1",
    )
    .bind(&challenge)
    .fetch_one(&*ctx.write)
    .await
    .expect("count");
    assert_eq!(count, 0, "the rejected challenge must not reach the table");
}

#[tokio::test]
async fn cleanup_physically_deletes_expired_challenges_and_spares_live_ones() {
    let ctx = setup("wa-cleanup").await;
    let live = format!("live-{}", Uuid::new_v4().simple());
    let stale = format!("stale-{}", Uuid::new_v4().simple());
    for (challenge, ttl) in [(&live, 3600u64), (&stale, 1u64)] {
        ctx.repo
            .store_webauthn_challenge(StoreChallengeParams {
                challenge,
                kind: WebAuthnChallengeKind::Authentication,
                user_id: Some(&ctx.user_id),
                state: &serde_json::Value::Null,
                oauth_state: None,
                ttl: Duration::from_secs(ttl),
            })
            .await
            .expect("store");
    }
    sqlx::query("UPDATE webauthn_challenges SET expires_at = NOW() - INTERVAL '1 hour' WHERE challenge = $1")
        .bind(&stale)
        .execute(&*ctx.write)
        .await
        .expect("age the stale challenge");

    ctx.cleanup
        .delete_expired_webauthn_challenges()
        .await
        .expect("cleanup");

    let remaining = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM webauthn_challenges WHERE challenge = ANY($1)",
    )
    .bind(vec![stale.clone()])
    .fetch_one(&*ctx.write)
    .await
    .expect("count stale");
    assert_eq!(
        remaining, 0,
        "cleanup must delete the row; consume() reports None either way"
    );

    let survivor = ctx
        .repo
        .consume_webauthn_challenge(&live, WebAuthnChallengeKind::Authentication)
        .await
        .expect("consume");
    assert!(survivor.is_some(), "a live challenge must survive cleanup");
}
