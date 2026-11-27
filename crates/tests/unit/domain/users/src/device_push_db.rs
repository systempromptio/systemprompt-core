//! DB-backed tests for `DevicePushService`: a caller writes only its own
//! device's token, a revoked device cannot register, a refresh replaces the
//! token, revocation and the user purge remove tokens, the admin list shows
//! what a device reported, and the push token directory returns only the
//! live tokens of unrevoked devices and revokes a token by its value.

use std::sync::Arc;

use systemprompt_extension::notifications::{DevicePushToken, PushTokenDirectory};
use systemprompt_identifiers::UserId;
use systemprompt_models::devices::{AppIdentifier, DeviceClientInfo, DevicePlatform};
use systemprompt_models::notifications::{PushProvider, PushToken, RegisterPushTokenRequest};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use systemprompt_users::{
    DeviceCertService, DevicePushError, DevicePushService, EnrollDeviceCertServiceParams,
    UserDeviceCert, UserRepository,
};

const APP_ID: &str = "io.example.app";

struct Ctx {
    push: DevicePushService,
    certs: DeviceCertService,
    repo: UserRepository,
    owner: UserId,
    other: UserId,
}

async fn setup(prefix: &str) -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = UserRepository::new(&pool);
    let shared = Arc::new(UserRepository::new(&pool));
    let owner = unique_user_id(prefix);
    let other = unique_user_id(prefix);
    for user in [&owner, &other] {
        seed_user_row(&pool, user, &format!("{}@dps.invalid", user.as_str()))
            .await
            .expect("seed user");
    }
    Ctx {
        push: DevicePushService::new(Arc::clone(&shared)),
        certs: DeviceCertService::new(shared),
        repo,
        owner,
        other,
    }
}

async fn cleanup(ctx: &Ctx) {
    for user in [&ctx.owner, &ctx.other] {
        let _ = ctx
            .repo
            .delete(user, &crate::purge_plan::purge_plan())
            .await;
    }
}

fn unique_fingerprint() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn unique_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

async fn enroll(ctx: &Ctx, user: &UserId, fingerprint: &str) -> UserDeviceCert {
    ctx.certs
        .enroll(EnrollDeviceCertServiceParams {
            user_id: user,
            fingerprint,
            label: "phone",
        })
        .await
        .expect("enrol device")
}

fn ios(token: &str) -> RegisterPushTokenRequest {
    RegisterPushTokenRequest {
        provider: PushProvider::Apns,
        token: PushToken::try_new(token).expect("push token"),
        client: DeviceClientInfo {
            platform: DevicePlatform::Ios,
            os_version: "18.2".to_owned(),
            app_version: "1.4.0".parse().expect("app version"),
            app_id: AppIdentifier::try_new(APP_ID).expect("app id"),
        },
    }
}

async fn live_tokens(ctx: &Ctx, user: &UserId) -> Vec<String> {
    ctx.push
        .tokens_for(user)
        .await
        .expect("directory answers")
        .into_iter()
        .map(|token| token.token.expose().to_owned())
        .collect()
}

#[tokio::test]
async fn a_device_registers_its_own_token() {
    let ctx = setup("dps1").await;
    let fingerprint = unique_fingerprint();
    let device = enroll(&ctx, &ctx.owner, &fingerprint).await;
    let token = unique_token();

    let registration = ctx
        .push
        .register(&ctx.owner, &fingerprint.to_ascii_uppercase(), &ios(&token))
        .await
        .expect("register");
    assert_eq!(registration.device_id.as_str(), device.id.as_str());
    assert_eq!(registration.provider, PushProvider::Apns);

    let tokens = ctx.push.tokens_for(&ctx.owner).await.expect("tokens");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].device_id.as_str(), device.id.as_str());
    assert_eq!(tokens[0].provider, PushProvider::Apns);
    assert_eq!(tokens[0].token.expose(), token);
    assert_eq!(tokens[0].app_id.as_str(), APP_ID);

    cleanup(&ctx).await;
}

#[tokio::test]
async fn another_user_cannot_write_a_devices_token() {
    let ctx = setup("dps2").await;
    let fingerprint = unique_fingerprint();
    enroll(&ctx, &ctx.owner, &fingerprint).await;
    let owner_token = unique_token();
    ctx.push
        .register(&ctx.owner, &fingerprint, &ios(&owner_token))
        .await
        .expect("owner registers");

    let err = ctx
        .push
        .register(&ctx.other, &fingerprint, &ios(&unique_token()))
        .await
        .expect_err("a foreign caller is refused");
    assert!(matches!(err, DevicePushError::DeviceNotEnrolled), "{err}");
    let err = ctx
        .push
        .revoke(&ctx.other, &fingerprint)
        .await
        .expect_err("a foreign caller cannot revoke");
    assert!(matches!(err, DevicePushError::DeviceNotEnrolled), "{err}");

    assert_eq!(live_tokens(&ctx, &ctx.owner).await, vec![owner_token]);
    assert!(live_tokens(&ctx, &ctx.other).await.is_empty());

    cleanup(&ctx).await;
}

#[tokio::test]
async fn an_unknown_or_malformed_fingerprint_is_refused() {
    let ctx = setup("dps3").await;

    let err = ctx
        .push
        .register(&ctx.owner, &unique_fingerprint(), &ios(&unique_token()))
        .await
        .expect_err("no device holds the fingerprint");
    assert!(matches!(err, DevicePushError::DeviceNotEnrolled), "{err}");
    let err = ctx
        .push
        .register(&ctx.owner, "not-a-fingerprint", &ios(&unique_token()))
        .await
        .expect_err("a malformed fingerprint is refused");
    assert!(
        matches!(err, DevicePushError::InvalidFingerprint(_)),
        "{err}"
    );

    cleanup(&ctx).await;
}

#[tokio::test]
async fn a_revoked_device_cannot_register_and_loses_its_token() {
    let ctx = setup("dps4").await;
    let fingerprint = unique_fingerprint();
    let device = enroll(&ctx, &ctx.owner, &fingerprint).await;
    ctx.push
        .register(&ctx.owner, &fingerprint, &ios(&unique_token()))
        .await
        .expect("register");

    assert!(
        ctx.certs
            .revoke(&device.id, &ctx.owner)
            .await
            .expect("revoke device")
    );
    assert!(
        live_tokens(&ctx, &ctx.owner).await.is_empty(),
        "revoking a device removes its token"
    );
    let stored: i64 =
        sqlx::query_scalar("SELECT count(*) FROM user_device_push_tokens WHERE device_id = $1")
            .bind(device.id.as_str())
            .fetch_one(test_db_pool().await.pool().as_ref())
            .await
            .expect("count tokens");
    assert_eq!(stored, 0, "the token row is deleted, not hidden");

    let err = ctx
        .push
        .register(&ctx.owner, &fingerprint, &ios(&unique_token()))
        .await
        .expect_err("a revoked device cannot register");
    assert!(matches!(err, DevicePushError::DeviceNotEnrolled), "{err}");
    assert!(live_tokens(&ctx, &ctx.owner).await.is_empty());

    cleanup(&ctx).await;
}

#[tokio::test]
async fn a_refresh_replaces_the_token() {
    let ctx = setup("dps5").await;
    let fingerprint = unique_fingerprint();
    enroll(&ctx, &ctx.owner, &fingerprint).await;
    let first = unique_token();
    let second = unique_token();

    let before = ctx
        .push
        .register(&ctx.owner, &fingerprint, &ios(&first))
        .await
        .expect("register");
    let after = ctx
        .push
        .register(&ctx.owner, &fingerprint, &ios(&second))
        .await
        .expect("refresh");
    assert_eq!(after.device_id, before.device_id);
    assert!(after.registered_at >= before.registered_at);
    assert_eq!(live_tokens(&ctx, &ctx.owner).await, vec![second]);

    cleanup(&ctx).await;
}

#[tokio::test]
async fn a_token_moves_to_the_device_that_registered_it_last() {
    let ctx = setup("dps6").await;
    let first = unique_fingerprint();
    let second = unique_fingerprint();
    enroll(&ctx, &ctx.owner, &first).await;
    enroll(&ctx, &ctx.other, &second).await;
    let token = unique_token();

    ctx.push
        .register(&ctx.owner, &first, &ios(&token))
        .await
        .expect("first holder");
    ctx.push
        .register(&ctx.other, &second, &ios(&token))
        .await
        .expect("second holder");

    assert!(
        live_tokens(&ctx, &ctx.owner).await.is_empty(),
        "the previous holder no longer receives pushes at that address"
    );
    assert_eq!(live_tokens(&ctx, &ctx.other).await, vec![token]);

    cleanup(&ctx).await;
}

#[tokio::test]
async fn revoking_the_token_removes_it_once() {
    let ctx = setup("dps7").await;
    let fingerprint = unique_fingerprint();
    enroll(&ctx, &ctx.owner, &fingerprint).await;
    ctx.push
        .register(&ctx.owner, &fingerprint, &ios(&unique_token()))
        .await
        .expect("register");

    let first = ctx
        .push
        .revoke(&ctx.owner, &fingerprint)
        .await
        .expect("revoke");
    assert!(first.revoked);
    let second = ctx
        .push
        .revoke(&ctx.owner, &fingerprint)
        .await
        .expect("revoke again");
    assert!(!second.revoked, "the device held none the second time");
    assert!(live_tokens(&ctx, &ctx.owner).await.is_empty());

    cleanup(&ctx).await;
}

#[tokio::test]
async fn the_directory_returns_only_live_tokens_of_the_users_devices() {
    let ctx = setup("dps8").await;
    let kept = unique_fingerprint();
    let revoked = unique_fingerprint();
    let dropped = unique_fingerprint();
    let foreign = unique_fingerprint();
    enroll(&ctx, &ctx.owner, &kept).await;
    let revoked_device = enroll(&ctx, &ctx.owner, &revoked).await;
    let dropped_device = enroll(&ctx, &ctx.owner, &dropped).await;
    enroll(&ctx, &ctx.other, &foreign).await;
    let kept_token = unique_token();
    for (user, fingerprint, token) in [
        (&ctx.owner, &kept, kept_token.clone()),
        (&ctx.owner, &revoked, unique_token()),
        (&ctx.owner, &dropped, unique_token()),
        (&ctx.other, &foreign, unique_token()),
    ] {
        ctx.push
            .register(user, fingerprint, &ios(&token))
            .await
            .expect("register");
    }
    assert_eq!(live_tokens(&ctx, &ctx.owner).await.len(), 3);

    ctx.certs
        .revoke(&revoked_device.id, &ctx.owner)
        .await
        .expect("revoke device");
    let directory: &dyn PushTokenDirectory = &ctx.push;
    let unregistered = ctx
        .push
        .tokens_for(&ctx.owner)
        .await
        .expect("tokens")
        .into_iter()
        .find(|token| token.device_id.as_str() == dropped_device.id.as_str())
        .expect("the third device still holds its token");
    directory
        .revoke(&DevicePushToken {
            provider: PushProvider::Fcm,
            ..unregistered.clone()
        })
        .await
        .expect("another provider's token is absent, which is not an error");
    assert_eq!(live_tokens(&ctx, &ctx.owner).await.len(), 2);
    directory
        .revoke(&unregistered)
        .await
        .expect("drop the unregistered token");

    assert_eq!(live_tokens(&ctx, &ctx.owner).await, vec![kept_token]);
    assert_eq!(live_tokens(&ctx, &ctx.other).await.len(), 1);

    cleanup(&ctx).await;
}

// Why: a provider's verdict is about the token it was sent. A device that
// registered a new token between the directory read and the verdict must
// keep it, or it stays unreachable until the app next registers.
#[tokio::test]
async fn a_refreshed_token_survives_the_revocation_of_the_one_it_replaced() {
    let ctx = setup("dps10").await;
    let fingerprint = unique_fingerprint();
    enroll(&ctx, &ctx.owner, &fingerprint).await;
    ctx.push
        .register(&ctx.owner, &fingerprint, &ios(&unique_token()))
        .await
        .expect("register");
    let stale = ctx
        .push
        .tokens_for(&ctx.owner)
        .await
        .expect("tokens")
        .pop()
        .expect("the device holds its first token");
    let refreshed = unique_token();
    ctx.push
        .register(&ctx.owner, &fingerprint, &ios(&refreshed))
        .await
        .expect("refresh");

    let directory: &dyn PushTokenDirectory = &ctx.push;
    directory
        .revoke(&stale)
        .await
        .expect("a token that is no longer held is not an error");

    assert_eq!(live_tokens(&ctx, &ctx.owner).await, vec![refreshed]);

    cleanup(&ctx).await;
}

#[tokio::test]
async fn purging_a_user_removes_their_tokens() {
    let ctx = setup("dps9").await;
    let fingerprint = unique_fingerprint();
    enroll(&ctx, &ctx.owner, &fingerprint).await;
    ctx.push
        .register(&ctx.owner, &fingerprint, &ios(&unique_token()))
        .await
        .expect("register");

    ctx.repo
        .delete(&ctx.owner, &crate::purge_plan::purge_plan())
        .await
        .expect("purge the user");

    let stored: i64 =
        sqlx::query_scalar("SELECT count(*) FROM user_device_push_tokens WHERE user_id = $1")
            .bind(ctx.owner.as_str())
            .fetch_one(test_db_pool().await.pool().as_ref())
            .await
            .expect("count tokens");
    assert_eq!(stored, 0);
    assert!(live_tokens(&ctx, &ctx.owner).await.is_empty());

    cleanup(&ctx).await;
}

#[tokio::test]
async fn the_admin_list_shows_platform_and_app_version() {
    let ctx = setup("dps10").await;
    let reported = unique_fingerprint();
    let silent = unique_fingerprint();
    let phone = enroll(&ctx, &ctx.owner, &reported).await;
    let laptop = enroll(&ctx, &ctx.owner, &silent).await;
    ctx.push
        .register(&ctx.owner, &reported, &ios(&unique_token()))
        .await
        .expect("register");

    let devices = ctx
        .push
        .list_devices(Some(&ctx.owner))
        .await
        .expect("admin list");
    assert_eq!(devices.len(), 2);
    let row = |id: &str| {
        devices
            .iter()
            .find(|device| device.device_id.as_str() == id)
            .expect("listed")
    };

    let phone = row(phone.id.as_str());
    assert_eq!(phone.user_id, ctx.owner);
    assert_eq!(phone.platform, DevicePlatform::Ios);
    assert_eq!(phone.os_version.as_deref(), Some("18.2"));
    assert_eq!(
        phone
            .app_version
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("1.4.0")
    );
    assert_eq!(
        phone.app_id.as_ref().map(AppIdentifier::as_str),
        Some(APP_ID)
    );
    assert_eq!(phone.push_provider, Some(PushProvider::Apns));
    assert!(phone.last_seen_at.is_some());
    assert!(phone.revoked_at.is_none());

    let laptop = row(laptop.id.as_str());
    assert_eq!(laptop.platform, DevicePlatform::Unknown);
    assert!(laptop.app_version.is_none());
    assert!(laptop.push_provider.is_none());
    assert!(laptop.last_seen_at.is_none());

    assert!(
        ctx.push
            .list_devices(Some(&ctx.other))
            .await
            .expect("admin list")
            .is_empty()
    );

    cleanup(&ctx).await;
}

#[tokio::test]
async fn client_info_the_provider_cannot_serve_is_refused() {
    let ctx = setup("dps11").await;
    let fingerprint = unique_fingerprint();
    enroll(&ctx, &ctx.owner, &fingerprint).await;

    let mut wrong_platform = ios(&unique_token());
    wrong_platform.client.platform = DevicePlatform::Android;
    let err = ctx
        .push
        .register(&ctx.owner, &fingerprint, &wrong_platform)
        .await
        .expect_err("apns does not serve android");
    assert!(
        matches!(err, DevicePushError::ProviderMismatch { .. }),
        "{err}"
    );

    let mut blank_os = ios(&unique_token());
    blank_os.client.os_version = " ".to_owned();
    let err = ctx
        .push
        .register(&ctx.owner, &fingerprint, &blank_os)
        .await
        .expect_err("a blank operating-system version is refused");
    assert!(matches!(err, DevicePushError::InvalidOsVersion), "{err}");
    assert!(live_tokens(&ctx, &ctx.owner).await.is_empty());

    cleanup(&ctx).await;
}
