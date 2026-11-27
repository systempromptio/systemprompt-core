use std::time::Duration;

use serde_json::json;
use systemprompt_device::accounts::AccountsSnapshot;
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_device::view::{AccountStatus, AccountView, SyncStatus};
use systemprompt_identifiers::UserId;
use tokio::runtime::Handle;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use systemprompt_harness::sync::{Launch, SyncEngine};

use crate::sync::fake_ports::FakeStore;
use crate::sync::fake_target::FakeTarget;
use crate::sync::fakes::{BASE, FakeClock, fetched, key, manifest, version};

fn view(id: &str, active: bool) -> AccountView {
    AccountView {
        id: AccountId::new(id),
        label: id.to_owned(),
        url: BASE.to_owned(),
        user: None,
        org: None,
        status: AccountStatus::SignedIn,
        active,
        fingerprint: Some(key().fingerprint().clone()),
        pinned: Some(key().fingerprint().clone()),
        trusted: true,
    }
}

fn snapshot(active: Option<&str>) -> AccountsSnapshot {
    AccountsSnapshot {
        accounts: vec![
            view("a-1", active == Some("a-1")),
            view("a-2", active == Some("a-2")),
        ],
        active: active.map(AccountId::new),
        offers: Vec::new(),
        signing_in: None,
        last_signed_in: None,
        secure_storage: true,
    }
}

async fn settle(
    status: &mut watch::Receiver<SyncStatus>,
    wanted: impl Fn(&SyncStatus) -> bool + Send + Sync,
) {
    tokio::time::timeout(
        Duration::from_secs(5),
        status.wait_for(|value| wanted(value)),
    )
    .await
    .unwrap()
    .unwrap();
}

fn synced(account: &str) -> impl Fn(&SyncStatus) -> bool + '_ {
    move |status| matches!(status, SyncStatus::Synced { account: synced, .. } if synced.as_str() == account)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switching_accounts_clears_the_previous_one_and_syncs_the_next_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let store = FakeStore::default();
    store.state().pinned = Some(key());
    store.state().user = Some(UserId::new("u-1"));
    store
        .instance
        .state()
        .manifests
        .push_back(Ok(fetched(manifest(json!({})), "e1")));
    let target = FakeTarget::default();
    let (accounts, snapshots) = watch::channel(snapshot(Some("a-1")));
    let launch = Launch {
        store: store.clone(),
        target: target.clone(),
        snapshots,
        clock: FakeClock::new(),
        state_dir: dir.path().join("state"),
        cache_dir: dir.path().join("cache"),
        device_label: "Harness on WS-01".to_owned(),
        release_version: "1.2.3".to_owned(),
        cancel: CancellationToken::new(),
    };
    let engine = SyncEngine::launch(launch, &Handle::current());
    let mut status = engine.subscribe();
    settle(&mut status, synced("a-1")).await;
    assert_eq!(target.state().clears, 0);

    engine.sync_now();
    for _ in 0..100 {
        if store
            .instance
            .state()
            .manifest_calls
            .iter()
            .any(|(_, no_cache)| *no_cache)
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        store
            .instance
            .state()
            .manifest_calls
            .iter()
            .any(|(_, no_cache)| *no_cache)
    );

    let second = manifest(json!({ "manifest_version": version(0, "0000000000000002") }));
    store
        .instance
        .state()
        .manifests
        .push_back(Ok(fetched(second, "e2")));
    accounts.send_replace(snapshot(Some("a-2")));
    settle(&mut status, synced("a-2")).await;
    assert_eq!(target.state().clears, 1);
    assert_eq!(target.state().applies, 2);

    accounts.send_replace(snapshot(None));
    settle(&mut status, |value| *value == SyncStatus::Idle).await;
    assert_eq!(target.state().clears, 2);
    engine.shutdown().await;
}
