use std::time::Duration;

use serde_json::json;
use systemprompt_device::accounts::AccountsSnapshot;
use systemprompt_device::client::{ClientError, ManifestFetch};
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_device::view::{AccountStatus, AccountView, SyncErrorKind, SyncStatus};
use systemprompt_identifiers::UserId;
use tokio::runtime::Handle;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use systemprompt_harness::sync::{Launch, SyncEngine};

use crate::sync::fake_ports::FakeStore;
use crate::sync::fake_target::FakeTarget;
use crate::sync::fakes::{BASE, FakeClock, fetched, key, manifest, status_error};

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

struct Rig {
    store: FakeStore,
    target: FakeTarget,
    accounts: watch::Sender<AccountsSnapshot>,
    engine: SyncEngine,
    status: watch::Receiver<SyncStatus>,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn start(first: Vec<Result<ManifestFetch, ClientError>>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = FakeStore::default();
        store.state().pinned = Some(key());
        store.state().user = Some(UserId::new("u-1"));
        store.instance.state().manifests.extend(first);
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
        let status = engine.subscribe();
        Self {
            store,
            target,
            accounts,
            engine,
            status,
            _dir: dir,
        }
    }

    async fn settle(&mut self, wanted: impl Fn(&SyncStatus) -> bool + Send + Sync) {
        tokio::time::timeout(
            Duration::from_secs(5),
            self.status.wait_for(|value| wanted(value)),
        )
        .await
        .unwrap()
        .unwrap();
    }

    async fn sign_out(&self, clears: usize) {
        self.accounts.send_replace(snapshot(None));
        for _ in 0..100 {
            if self.target.state().clears >= clears {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(self.target.state().clears, clears);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn refusal() -> Vec<Result<ManifestFetch, ClientError>> {
    vec![Err(status_error(401)), Err(status_error(401))]
}

fn refused(account: &str) -> impl Fn(&SyncStatus) -> bool + '_ {
    move |status| {
        matches!(
            status,
            SyncStatus::Error { account: refused, kind: SyncErrorKind::Refused, .. }
                if refused.as_str() == account
        )
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refusal_stays_reported_when_the_sync_goes_idle() {
    let mut rig = Rig::start(refusal());
    rig.settle(refused("a-1")).await;

    rig.sign_out(1).await;
    assert!(refused("a-1")(&rig.status.borrow()));
    rig.engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_later_successful_sync_clears_a_refusal() {
    let mut rig = Rig::start(refusal());
    rig.settle(refused("a-1")).await;
    rig.sign_out(1).await;

    rig.store
        .instance
        .state()
        .manifests
        .push_back(Ok(fetched(manifest(json!({})), "e1")));
    rig.accounts.send_replace(snapshot(Some("a-1")));
    rig.settle(
        |status| matches!(status, SyncStatus::Synced { account, .. } if account.as_str() == "a-1"),
    )
    .await;
    rig.engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_later_refusal_replaces_an_earlier_one() {
    let mut rig = Rig::start(refusal());
    rig.settle(refused("a-1")).await;

    rig.store.instance.state().manifests.extend(refusal());
    rig.accounts.send_replace(snapshot(Some("a-2")));
    rig.settle(refused("a-2")).await;
    rig.engine.shutdown().await;
}
