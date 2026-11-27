use std::sync::Arc;

use systemprompt_device::accounts::{AccountsError, ProbeResult};
use systemprompt_device::client::{Freshness, ManifestKey};
use systemprompt_device::secure::MemorySealer;
use systemprompt_device::view::{AccountStatus, SignInFailure, SignInOutcome};
use tokio::net::TcpListener;

use crate::accounts::support::{
    CODE, FakeBrowser, FakeInstance, PAT, TOKEN, open, open_plain, store_path,
};

#[tokio::test]
async fn a_pat_sign_in_stores_a_sealed_account_and_only_offers_the_key() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let accounts = open_plain(&store_path(&dir));

    let outcome = accounts
        .sign_in_pat(&instance.url, format!("  {PAT}\n"), true)
        .await;
    assert_eq!(outcome, SignInOutcome::SignedIn);

    let listed = accounts.list();
    assert_eq!(listed.len(), 1);
    let account = &listed[0];
    assert_eq!(account.status, AccountStatus::SignedIn);
    assert!(account.active);
    assert_eq!(account.user.as_deref(), Some("dev@example.test"));
    let offered = ManifestKey::from_raw([7u8; 32]);
    assert_eq!(account.fingerprint.as_ref(), Some(offered.fingerprint()));
    assert!(account.pinned.is_none());
    assert!(!account.trusted);

    let snapshot = accounts.snapshot();
    assert_eq!(snapshot.active.as_ref(), Some(&account.id));
    assert_eq!(snapshot.last_signed_in.as_ref(), Some(&account.id));
    assert_eq!(snapshot.offers.len(), 1);
    assert!(accounts.pinned_key(&account.id).unwrap().is_none());

    let on_disk = std::fs::read_to_string(store_path(&dir)).unwrap();
    assert!(!on_disk.contains(PAT));
    assert!(!on_disk.contains(TOKEN));

    let other = ManifestKey::from_raw([9u8; 32]);
    assert!(matches!(
        accounts.trust(&account.id, other.fingerprint()),
        Err(AccountsError::NothingToTrust { .. })
    ));
    accounts.trust(&account.id, offered.fingerprint()).unwrap();
    assert_eq!(accounts.pinned_key(&account.id).unwrap(), Some(offered));
    assert!(accounts.get(&account.id).unwrap().trusted);
    assert_eq!(
        accounts.snapshot().offers,
        Vec::<systemprompt_device::accounts::PendingKeyOffer>::new()
    );

    let session = accounts.session(&account.id, Freshness::Any).await.unwrap();
    assert_eq!(session.jwt.expose(), TOKEN);
    assert!(
        accounts
            .instance_key(&account.id)
            .unwrap()
            .as_str()
            .starts_with("127-0-0-1-")
    );
}

#[tokio::test]
async fn without_keep_the_pat_is_never_written_and_the_account_is_signed_out_next_launch() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let accounts = open_plain(&store_path(&dir));
    let outcome = accounts
        .sign_in_pat(&instance.url, PAT.to_owned(), false)
        .await;
    assert_eq!(outcome, SignInOutcome::SignedIn);
    let id = accounts.active_id().unwrap();
    assert_eq!(accounts.get(&id).unwrap().status, AccountStatus::SignedIn);
    assert!(accounts.session(&id, Freshness::Fresh).await.is_ok());
    let on_disk = std::fs::read_to_string(store_path(&dir)).unwrap();
    assert!(!on_disk.contains("\"pat\""));
    accounts.shutdown();

    let reopened = open_plain(&store_path(&dir));
    let account = reopened.get(&id).unwrap();
    assert_eq!(account.status, AccountStatus::SignedOut);
    assert!(!account.active);
    assert!(reopened.snapshot().needs_sign_in());
}

#[tokio::test]
async fn a_refused_pat_or_missing_secure_storage_fails_the_sign_in() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let accounts = open_plain(&store_path(&dir));
    let refused = SignInOutcome::Failed(SignInFailure::Refused);
    assert_eq!(
        accounts
            .sign_in_pat(&instance.url, "sp-live-wrong".to_owned(), true)
            .await,
        refused
    );
    assert_eq!(
        accounts
            .sign_in_pat(&instance.url, "   ".to_owned(), true)
            .await,
        refused
    );
    assert_eq!(
        accounts.list(),
        Vec::<systemprompt_device::view::AccountView>::new()
    );

    let insecure_dir = tempfile::tempdir().unwrap();
    let insecure = open(
        &store_path(&insecure_dir),
        Arc::new(MemorySealer::disabled()),
        Arc::default(),
    );
    assert_eq!(
        insecure
            .sign_in_pat(&instance.url, PAT.to_owned(), true)
            .await,
        SignInOutcome::Failed(SignInFailure::Insecure)
    );
    assert!(!insecure.snapshot().secure_storage);
}

#[tokio::test]
async fn a_pasted_code_is_exchanged_for_a_pat() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let browser = Arc::new(FakeBrowser::default());
    let accounts = open(
        &store_path(&dir),
        Arc::new(MemorySealer::for_tests()),
        Arc::clone(&browser),
    );
    accounts.open_code_page(&instance.url).await.unwrap();
    let page = browser.opened.lock().unwrap()[0].clone();
    assert_eq!(page.path(), "/bridge-auth/device-link");
    assert!(page.query().is_none());
    assert_eq!(
        accounts
            .sign_in_code(&instance.url, "nope".to_owned(), true)
            .await,
        SignInOutcome::Failed(SignInFailure::Refused)
    );
    assert_eq!(
        accounts
            .sign_in_code(&instance.url, CODE.to_owned(), true)
            .await,
        SignInOutcome::SignedIn
    );
}

#[tokio::test]
async fn a_browser_sign_in_completes_through_the_loopback_callback() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let browser = Arc::new(FakeBrowser {
        answer_with: Some(CODE),
        ..FakeBrowser::default()
    });
    let accounts = open(
        &store_path(&dir),
        Arc::new(MemorySealer::for_tests()),
        Arc::clone(&browser),
    );
    let outcome = accounts
        .sign_in_browser(&instance.url, true)
        .outcome()
        .await;
    assert_eq!(outcome, SignInOutcome::SignedIn);
    let opened = browser.opened.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    assert!(
        opened[0]
            .query_pairs()
            .any(|(key, value)| { key == "redirect" && value.starts_with("http://127.0.0.1:") })
    );
    assert!(accounts.snapshot().signing_in.is_none());
    assert_eq!(accounts.list().len(), 1);
}

#[tokio::test]
async fn a_browser_sign_in_can_be_cancelled() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let accounts = open_plain(&store_path(&dir));
    let sign_in = accounts.sign_in_browser(&instance.url, true);
    let cancel = sign_in.canceller();
    let waiting = tokio::spawn(sign_in.outcome());
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    cancel.cancel();
    assert_eq!(waiting.await.unwrap(), SignInOutcome::Cancelled);
    assert_eq!(
        accounts.list(),
        Vec::<systemprompt_device::view::AccountView>::new()
    );

    let first = accounts.sign_in_browser(&instance.url, true);
    let second = accounts.sign_in_browser(&instance.url, true);
    assert_eq!(first.outcome().await, SignInOutcome::Cancelled);
    accounts.cancel_sign_in();
    assert_eq!(second.outcome().await, SignInOutcome::Cancelled);
}

#[tokio::test]
async fn probing_tells_instances_from_other_addresses() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let accounts = open_plain(&store_path(&dir));
    assert_eq!(accounts.probe("not a url").await, ProbeResult::Malformed);
    assert_eq!(
        accounts.probe("http://remote.example.com").await,
        ProbeResult::Malformed
    );
    let ProbeResult::Instance(found) = accounts.probe(&format!("{}/", instance.url)).await else {
        panic!("the fake instance is an instance");
    };
    assert_eq!(found.url, instance.url);
    assert_eq!(found.modes, ["pat", "session"]);
    assert_eq!(found.version.as_deref(), Some("0.90.0"));
    assert!(!found.browser);
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    assert_eq!(
        accounts.probe(&format!("http://127.0.0.1:{port}")).await,
        ProbeResult::Unreachable
    );
}
