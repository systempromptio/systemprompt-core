//! DB-backed tests for `DeviceCertService::enroll_or_reuse`: the same user
//! presenting the same fingerprint gets the cert back, another user is
//! refused rather than silently reassigned.

use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use systemprompt_users::{
    DEVICE_FINGERPRINT_FOREIGN_USER, DeviceCertService, EnrollDeviceCertServiceParams, UserError,
    UserRepository,
};

struct Ctx {
    service: DeviceCertService,
    repo: UserRepository,
    owner: UserId,
    other: UserId,
}

async fn setup(prefix: &str) -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = UserRepository::new(&pool);
    let service = DeviceCertService::new(Arc::new(UserRepository::new(&pool)));
    let owner = unique_user_id(prefix);
    let other = unique_user_id(prefix);
    for user in [&owner, &other] {
        seed_user_row(&pool, user, &format!("{}@dcr.invalid", user.as_str()))
            .await
            .expect("seed user");
    }
    Ctx {
        service,
        repo,
        owner,
        other,
    }
}

async fn cleanup(ctx: &Ctx) {
    // user_device_certs.user_id is FK ON DELETE CASCADE.
    let _ = ctx.repo.delete(&ctx.owner).await;
    let _ = ctx.repo.delete(&ctx.other).await;
}

// Why: fingerprints are globally unique, so each test mints its own; two
// uuids give 64 hex chars, the SHA-256 width the service demands.
fn unique_fingerprint() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

#[tokio::test]
async fn same_user_and_fingerprint_returns_the_existing_cert() {
    let ctx = setup("dcr1").await;
    let fingerprint = unique_fingerprint();
    let first = ctx
        .service
        .enroll_or_reuse(EnrollDeviceCertServiceParams {
            user_id: &ctx.owner,
            fingerprint: &fingerprint,
            label: "laptop",
        })
        .await
        .expect("first enrolment");
    let second = ctx
        .service
        .enroll_or_reuse(EnrollDeviceCertServiceParams {
            user_id: &ctx.owner,
            fingerprint: &fingerprint.to_ascii_uppercase(),
            label: "renamed laptop",
        })
        .await
        .expect("reuse");
    assert_eq!(second.id, first.id);
    assert_eq!(second.label, first.label, "reuse keeps the original label");
    assert_eq!(
        ctx.service
            .list_for_user(&ctx.owner)
            .await
            .expect("list")
            .len(),
        1
    );

    cleanup(&ctx).await;
}

#[tokio::test]
async fn another_user_presenting_the_fingerprint_is_refused() {
    let ctx = setup("dcr2").await;
    let fingerprint = unique_fingerprint();
    ctx.service
        .enroll_or_reuse(EnrollDeviceCertServiceParams {
            user_id: &ctx.owner,
            fingerprint: &fingerprint,
            label: "laptop",
        })
        .await
        .expect("owner enrolment");
    let err = ctx
        .service
        .enroll_or_reuse(EnrollDeviceCertServiceParams {
            user_id: &ctx.other,
            fingerprint: &fingerprint,
            label: "laptop",
        })
        .await
        .expect_err("foreign user must be refused");
    match err {
        UserError::Validation(message) => assert_eq!(message, DEVICE_FINGERPRINT_FOREIGN_USER),
        other => panic!("expected validation error, got {other:?}"),
    }
    assert!(
        ctx.service
            .list_for_user(&ctx.other)
            .await
            .expect("list")
            .is_empty()
    );

    cleanup(&ctx).await;
}
