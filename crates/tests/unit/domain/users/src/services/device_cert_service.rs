//! DB-backed tests for `DeviceCertService` (enroll/verify/list/revoke plus
//! fingerprint + label validation).

use std::sync::Arc;
use systemprompt_identifiers::{DeviceCertId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use systemprompt_users::{
    DeviceCertService, EnrollDeviceCertServiceParams, UserError, UserRepository,
};

struct Ctx {
    service: DeviceCertService,
    repo: UserRepository,
    user_id: UserId,
}

async fn setup(prefix: &str) -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let service = DeviceCertService::new(Arc::new(
        UserRepository::new(&pool).expect("user repository"),
    ));
    let repo = UserRepository::new(&pool).expect("repo");
    let user_id = unique_user_id(prefix);
    seed_user_row(
        &pool,
        &user_id,
        &format!("{}@dcs.invalid", user_id.as_str()),
    )
    .await
    .expect("seed user");
    Ctx {
        service,
        repo,
        user_id,
    }
}

async fn cleanup(ctx: &Ctx) {
    // user_device_certs.user_id is FK ON DELETE CASCADE.
    let _ = ctx.repo.delete(&ctx.user_id).await;
}

fn valid_fingerprint(seed: char) -> String {
    std::iter::repeat_n(seed, 64).collect()
}

#[tokio::test]
async fn enroll_normalizes_and_verify_roundtrips() {
    let ctx = setup("dcs1").await;
    // Upper-case + surrounding whitespace must normalize to lower-case.
    let raw = format!("  {}  ", valid_fingerprint('A'));

    let enrolled = ctx
        .service
        .enroll(EnrollDeviceCertServiceParams {
            user_id: &ctx.user_id,
            fingerprint: &raw,
            label: "  work laptop  ",
        })
        .await
        .expect("enroll");
    assert_eq!(enrolled.fingerprint, valid_fingerprint('a'));
    assert_eq!(enrolled.label, "work laptop");

    let verified = ctx
        .service
        .verify(&valid_fingerprint('a'))
        .await
        .expect("verify")
        .expect("present");
    assert_eq!(verified.id, enrolled.id);

    let listed = ctx.service.list_for_user(&ctx.user_id).await.expect("list");
    assert_eq!(listed.len(), 1);

    let revoked = ctx
        .service
        .revoke(&enrolled.id, &ctx.user_id)
        .await
        .expect("revoke");
    assert!(revoked);
    assert!(
        ctx.service
            .verify(&valid_fingerprint('a'))
            .await
            .expect("verify after revoke")
            .is_none()
    );

    cleanup(&ctx).await;
}

#[tokio::test]
async fn enroll_rejects_empty_label() {
    let ctx = setup("dcs2").await;
    let err = ctx
        .service
        .enroll(EnrollDeviceCertServiceParams {
            user_id: &ctx.user_id,
            fingerprint: &valid_fingerprint('b'),
            label: "   ",
        })
        .await
        .expect_err("empty label must fail");
    assert!(matches!(err, UserError::Validation(_)));

    cleanup(&ctx).await;
}

#[tokio::test]
async fn enroll_rejects_bad_length_fingerprint() {
    let ctx = setup("dcs3").await;
    let err = ctx
        .service
        .enroll(EnrollDeviceCertServiceParams {
            user_id: &ctx.user_id,
            fingerprint: "deadbeef",
            label: "laptop",
        })
        .await
        .expect_err("short fingerprint must fail");
    assert!(matches!(err, UserError::Validation(_)));

    cleanup(&ctx).await;
}

#[tokio::test]
async fn enroll_rejects_non_hex_fingerprint() {
    let ctx = setup("dcs4").await;
    let non_hex: String = std::iter::repeat_n('z', 64).collect();
    let err = ctx
        .service
        .enroll(EnrollDeviceCertServiceParams {
            user_id: &ctx.user_id,
            fingerprint: &non_hex,
            label: "laptop",
        })
        .await
        .expect_err("non-hex fingerprint must fail");
    assert!(matches!(err, UserError::Validation(_)));

    cleanup(&ctx).await;
}

#[tokio::test]
async fn verify_rejects_invalid_fingerprint() {
    let ctx = setup("dcs5").await;
    let err = ctx
        .service
        .verify("not-a-valid-fingerprint")
        .await
        .expect_err("invalid fingerprint must fail");
    assert!(matches!(err, UserError::Validation(_)));

    cleanup(&ctx).await;
}

#[tokio::test]
async fn revoke_unknown_returns_false() {
    let ctx = setup("dcs6").await;
    let revoked = ctx
        .service
        .revoke(&DeviceCertId::generate(), &ctx.user_id)
        .await
        .expect("revoke");
    assert!(!revoked);

    cleanup(&ctx).await;
}
