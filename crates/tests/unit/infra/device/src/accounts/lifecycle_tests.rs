use std::sync::atomic::Ordering;

use systemprompt_device::accounts::{AccountsError, DeviceEntry};
use systemprompt_device::client::{CredentialError, Freshness};
use systemprompt_device::protocol::DeviceCredential;
use systemprompt_device::view::{AccountStatus, SignInOutcome};
use systemprompt_identifiers::{DeviceId, UserId};
use systemprompt_models::bridge::manifest_version::ManifestVersion;

use crate::accounts::support::{FakeInstance, PAT, open_plain, store_path};

#[tokio::test]
async fn a_refused_pat_signs_the_account_out_and_keeps_the_row() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let accounts = open_plain(&store_path(&dir));
    let outcome = accounts
        .sign_in_pat(&instance.url, PAT.to_owned(), true)
        .await;
    assert_eq!(outcome, SignInOutcome::SignedIn);
    let id = accounts.active_id().unwrap();
    let mut renewals = accounts.renewals();
    assert!(accounts.session(&id, Freshness::Fresh).await.is_ok());
    assert_eq!(renewals.recv().await.unwrap().account, id);

    instance.refuse_pat.store(true, Ordering::SeqCst);
    let refused = accounts.session(&id, Freshness::Fresh).await;
    assert!(matches!(refused, Err(CredentialError::Refused)));
    let account = accounts.get(&id).unwrap();
    assert_eq!(account.status, AccountStatus::SignedOut);
    assert!(!account.active);
    let signed_out = accounts.session(&id, Freshness::Any).await;
    assert!(matches!(signed_out, Err(CredentialError::SignedOut)));
    assert!(matches!(
        accounts.activate(&id),
        Err(AccountsError::NotSignedIn { .. })
    ));

    accounts.remove(&id).unwrap();
    assert_eq!(
        accounts.list(),
        Vec::<systemprompt_device::view::AccountView>::new()
    );
    assert!(matches!(
        accounts.remove(&id),
        Err(AccountsError::UnknownAccount { .. })
    ));
}

#[tokio::test]
async fn device_credentials_are_sealed_and_forgotten_on_sign_out() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let accounts = open_plain(&store_path(&dir));
    let outcome = accounts
        .sign_in_pat(&instance.url, PAT.to_owned(), true)
        .await;
    assert_eq!(outcome, SignInOutcome::SignedIn);
    let id = accounts.active_id().unwrap();
    let device = DeviceEntry {
        device_id: DeviceId::try_new("dev-1").unwrap(),
        user: UserId::new("user-1"),
        installation: Some("inst-1".to_owned()),
        credential: DeviceCredential::new("sp_device_secretvalue").unwrap(),
    };
    accounts.set_device(&id, &device).unwrap();
    accounts
        .set_outbox(&id, Some("outbox-1.json".to_owned()))
        .unwrap();
    let on_disk = std::fs::read_to_string(store_path(&dir)).unwrap();
    assert!(!on_disk.contains("sp_device_secretvalue"));
    let held = accounts.device(&id).unwrap().unwrap();
    assert_eq!(held.credential.expose(), "sp_device_secretvalue");
    assert_eq!(held.device_id.as_str(), "dev-1");
    assert_eq!(
        accounts.outbox(&id).unwrap().as_deref(),
        Some("outbox-1.json")
    );

    let version = ManifestVersion::try_new("2026-11-01T09:12:31Z-000001a19661c117").unwrap();
    accounts.record_applied_version(&id, &version).unwrap();
    assert_eq!(accounts.last_applied_version(&id).unwrap(), Some(version));

    accounts.sign_out(&id).unwrap();
    assert!(accounts.device(&id).unwrap().is_none());
    assert!(accounts.outbox(&id).unwrap().is_none());
    assert_eq!(accounts.get(&id).unwrap().status, AccountStatus::SignedOut);
}
