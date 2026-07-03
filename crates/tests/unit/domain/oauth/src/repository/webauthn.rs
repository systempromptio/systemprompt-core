// DB-backed WebAuthn credential persistence tests.

use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::{OAuthRepository, WebAuthnCredentialParams};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: OAuthRepository,
    user_id: UserId,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("wa");
    seed_user_row(&pool, &user_id, &format!("{}@wa.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    Some(Ctx { repo, user_id })
}

#[tokio::test]
async fn store_then_get_credentials() {
    let Some(ctx) = setup().await else { return };
    let id = format!("cred-{}", Uuid::new_v4());
    let credential_id = Uuid::new_v4().as_bytes().to_vec();
    let public_key = vec![1u8, 2, 3, 4];
    let transports = vec!["usb".to_owned(), "nfc".to_owned()];

    ctx.repo
        .store_webauthn_credential(
            WebAuthnCredentialParams::builder(&id, &ctx.user_id, &credential_id, &public_key, 0)
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
    assert_eq!(found.counter, 0);
    assert_eq!(found.display_name, "YubiKey");
    assert_eq!(found.device_type, "cross-platform");
    assert_eq!(found.transports, transports);
}

#[tokio::test]
async fn get_credentials_empty_for_unknown_user() {
    let Some(ctx) = setup().await else { return };
    let other = unique_user_id("wa-empty");
    let creds = ctx
        .repo
        .list_webauthn_credentials(&other)
        .await
        .expect("get");
    assert!(creds.is_empty());
}

#[tokio::test]
async fn update_counter_advances() {
    let Some(ctx) = setup().await else { return };
    let id = format!("cred-{}", Uuid::new_v4());
    let credential_id = Uuid::new_v4().as_bytes().to_vec();

    ctx.repo
        .store_webauthn_credential(
            WebAuthnCredentialParams::builder(&id, &ctx.user_id, &credential_id, &[9u8, 9, 9], 5)
                .with_device_type("platform")
                .build(),
        )
        .await
        .expect("store");

    ctx.repo
        .update_webauthn_credential_counter(&credential_id, 42)
        .await
        .expect("update counter");

    let creds = ctx
        .repo
        .list_webauthn_credentials(&ctx.user_id)
        .await
        .expect("get");
    let found = creds.iter().find(|c| c.id == id).expect("present");
    assert_eq!(found.counter, 42);
    found
        .last_used_at
        .expect("last_used_at set after counter update");
}
