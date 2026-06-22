//! DB-backed tests for the device-cert repository (enroll, find-active,
//! list-for-user, revoke).

use systemprompt_identifiers::{DeviceCertId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use systemprompt_users::{EnrollDeviceCertParams, UserRepository};
use uuid::Uuid;

struct Ctx {
    repo: UserRepository,
    user_id: UserId,
}

async fn setup(prefix: &str) -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = UserRepository::new(&pool).expect("repo");
    let user_id = unique_user_id(prefix);
    seed_user_row(&pool, &user_id, &format!("{}@dc.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    Some(Ctx { repo, user_id })
}

async fn cleanup(ctx: &Ctx) {
    // user_device_certs.user_id is FK ON DELETE CASCADE.
    let _ = ctx.repo.delete(&ctx.user_id).await;
}

fn fp() -> String {
    Uuid::new_v4().simple().to_string()
}

#[tokio::test]
async fn enroll_then_find_active_and_list() {
    let Some(ctx) = setup("dc1").await else {
        return;
    };
    let id = DeviceCertId::generate();
    let fingerprint = fp();

    let enrolled = ctx
        .repo
        .enroll_device_cert(EnrollDeviceCertParams {
            id: &id,
            user_id: &ctx.user_id,
            fingerprint: &fingerprint,
            label: "laptop",
        })
        .await
        .expect("enroll");
    assert_eq!(enrolled.id, id);
    assert_eq!(enrolled.user_id, ctx.user_id);
    assert_eq!(enrolled.label, "laptop");
    assert!(enrolled.revoked_at.is_none());

    let found = ctx
        .repo
        .find_active_device_cert_by_fingerprint(&fingerprint)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.id, id);

    let listed = ctx
        .repo
        .list_device_certs_for_user(&ctx.user_id)
        .await
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);

    cleanup(&ctx).await;
}

#[tokio::test]
async fn revoke_hides_from_active_lookup() {
    let Some(ctx) = setup("dc2").await else {
        return;
    };
    let id = DeviceCertId::generate();
    let fingerprint = fp();

    ctx.repo
        .enroll_device_cert(EnrollDeviceCertParams {
            id: &id,
            user_id: &ctx.user_id,
            fingerprint: &fingerprint,
            label: "phone",
        })
        .await
        .expect("enroll");

    let revoked = ctx
        .repo
        .revoke_device_cert(&id, &ctx.user_id)
        .await
        .expect("revoke");
    assert!(revoked);

    assert!(
        ctx.repo
            .find_active_device_cert_by_fingerprint(&fingerprint)
            .await
            .expect("find")
            .is_none(),
        "revoked cert must not resolve as active"
    );

    // Second revoke affects no rows (already revoked).
    let second = ctx
        .repo
        .revoke_device_cert(&id, &ctx.user_id)
        .await
        .expect("revoke again");
    assert!(!second);

    // Still listed (history retained), but flagged revoked.
    let listed = ctx
        .repo
        .list_device_certs_for_user(&ctx.user_id)
        .await
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert!(listed[0].revoked_at.is_some());

    cleanup(&ctx).await;
}

#[tokio::test]
async fn revoke_unknown_cert_returns_false() {
    let Some(ctx) = setup("dc3").await else {
        return;
    };
    let unknown = DeviceCertId::generate();
    let revoked = ctx
        .repo
        .revoke_device_cert(&unknown, &ctx.user_id)
        .await
        .expect("revoke");
    assert!(!revoked);

    cleanup(&ctx).await;
}

#[tokio::test]
async fn find_active_unknown_fingerprint_returns_none() {
    let Some(ctx) = setup("dc4").await else {
        return;
    };
    let found = ctx
        .repo
        .find_active_device_cert_by_fingerprint(&fp())
        .await
        .expect("find");
    assert!(found.is_none());

    cleanup(&ctx).await;
}
