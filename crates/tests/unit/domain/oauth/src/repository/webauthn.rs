// DB-backed WebAuthn credential persistence tests.

use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::{OAuthRepository, WebAuthnCredentialParams};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: OAuthRepository,
    user_id: UserId,
}

async fn setup() -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);
    let user_id = unique_user_id("wa");
    seed_user_row(&pool, &user_id, &format!("{}@wa.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    Ctx { repo, user_id }
}

#[tokio::test]
async fn store_then_get_credentials() {
    let ctx = setup().await;
    let id = format!("cred-{}", Uuid::new_v4());
    let credential_id = Uuid::new_v4().as_bytes().to_vec();
    let public_key = vec![1u8, 2, 3, 4];
    let transports = vec!["usb".to_owned(), "nfc".to_owned()];

    ctx.repo
        .store_webauthn_credential(
            WebAuthnCredentialParams::builder(&id, &ctx.user_id, &credential_id, &public_key)
                .with_display_name("YubiKey")
                .with_device_type("cross-platform")
                .with_transports(&transports)
                .build(),
        )
        .await
        .expect("store");

    let creds = ctx
        .repo
        .list_webauthn_credentials(&ctx.user_id)
        .await
        .expect("get");
    let found = creds.iter().find(|c| c.id == id).expect("present");
    assert_eq!(found.user_id, ctx.user_id);
    assert_eq!(found.credential_id, credential_id);
    assert_eq!(found.public_key, public_key);
    assert_eq!(found.display_name, "YubiKey");
    assert_eq!(found.device_type, "cross-platform");
    assert_eq!(found.transports, transports);
}

#[tokio::test]
async fn get_credentials_empty_for_unknown_user() {
    let ctx = setup().await;
    let other = unique_user_id("wa-empty");
    let creds = ctx
        .repo
        .list_webauthn_credentials(&other)
        .await
        .expect("get");
    assert!(creds.is_empty());
}

#[tokio::test]
async fn replace_passkey_swaps_the_blob_and_stamps_last_use() {
    let ctx = setup().await;
    let id = format!("cred-{}", Uuid::new_v4());
    let credential_id = Uuid::new_v4().as_bytes().to_vec();

    ctx.repo
        .store_webauthn_credential(
            WebAuthnCredentialParams::builder(&id, &ctx.user_id, &credential_id, &[9u8, 9, 9])
                .with_device_type("platform")
                .build(),
        )
        .await
        .expect("store");

    ctx.repo
        .replace_webauthn_passkey(&credential_id, &[9u8, 9, 9], &[4u8, 2])
        .await
        .expect("replace passkey");

    let creds = ctx
        .repo
        .list_webauthn_credentials(&ctx.user_id)
        .await
        .expect("get");
    let found = creds.iter().find(|c| c.id == id).expect("present");
    assert_eq!(found.public_key, vec![4u8, 2]);
    found
        .last_used_at
        .expect("last_used_at set after passkey update");
}

#[tokio::test]
async fn replace_passkey_refuses_a_stale_previous_blob() {
    let ctx = setup().await;
    let id = format!("cred-{}", Uuid::new_v4());
    let credential_id = Uuid::new_v4().as_bytes().to_vec();

    ctx.repo
        .store_webauthn_credential(
            WebAuthnCredentialParams::builder(&id, &ctx.user_id, &credential_id, &[1u8, 1])
                .with_device_type("platform")
                .build(),
        )
        .await
        .expect("store");

    ctx.repo
        .replace_webauthn_passkey(&credential_id, &[7u8, 7], &[2u8, 2])
        .await
        .expect_err("a concurrent update must not be overwritten");

    let creds = ctx
        .repo
        .list_webauthn_credentials(&ctx.user_id)
        .await
        .expect("get");
    let found = creds.iter().find(|c| c.id == id).expect("present");
    assert_eq!(found.public_key, vec![1u8, 1]);
    assert!(found.last_used_at.is_none());
}

#[tokio::test]
async fn touch_rejects_an_unknown_credential() {
    let ctx = setup().await;
    ctx.repo
        .touch_webauthn_credential(Uuid::new_v4().as_bytes())
        .await
        .expect_err("touching an unregistered credential must fail");
}
