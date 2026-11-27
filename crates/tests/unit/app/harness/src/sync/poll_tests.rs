use std::sync::Arc;

use serde_json::json;
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_device::view::{SyncErrorKind, SyncStatus};
use systemprompt_harness::extension::ExtensionId;
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use tokio::sync::watch;

use systemprompt_harness::sync::bundles::BundleCache;
use systemprompt_harness::sync::planner::{Outcome, Planner, Trigger};
use systemprompt_harness::sync::poll::{Poller, Scope};
use systemprompt_harness::sync::runtime::RuntimeFiles;

use crate::sync::fake_ports::FakeStore;
use crate::sync::fake_target::FakeTarget;
use crate::sync::fakes::{FakeClock, fetched, key, manifest, unreachable, version};

pub(crate) struct Rig {
    pub(crate) store: FakeStore,
    pub(crate) target: FakeTarget,
    pub(crate) poller: Poller<FakeStore, FakeTarget>,
    pub(crate) planner: Planner,
    pub(crate) status: watch::Receiver<SyncStatus>,
    _dir: tempfile::TempDir,
}

pub(crate) fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let store = FakeStore::default();
    store.state().pinned = Some(key());
    store.state().user = Some(UserId::new("u-1"));
    let target = FakeTarget::default();
    let status = Arc::new(watch::Sender::new(SyncStatus::Idle));
    let receiver = status.subscribe();
    let scope = Scope {
        account: AccountId::new("a-1"),
        store: store.clone(),
        target: target.clone(),
        instance: store.instance.clone(),
        clock: FakeClock::new(),
        status,
    };
    let poller = Poller::new(
        scope,
        BundleCache::new(dir.path()),
        RuntimeFiles::new(dir.path(), "Harness on WS-01"),
    );
    Rig {
        store,
        target,
        poller,
        planner: Planner::new(),
        status: receiver,
        _dir: dir,
    }
}

impl Rig {
    pub(crate) fn serve(
        &self,
        answer: Result<
            systemprompt_device::client::ManifestFetch,
            systemprompt_device::client::ClientError,
        >,
    ) {
        self.store.instance.state().manifests.push_back(answer);
    }

    pub(crate) async fn poll(&mut self, trigger: Trigger) -> Outcome {
        self.planner.request(trigger);
        let request = self.planner.take().unwrap_or_default();
        self.poller.poll(request, &mut self.planner).await
    }
}

#[tokio::test]
async fn a_304_applies_nothing_a_change_applies_once_and_a_resigned_copy_applies_nothing() {
    let mut rig = rig();
    rig.serve(Ok(fetched(manifest(json!({})), "e1")));
    assert_eq!(rig.poll(Trigger::Start).await, Outcome::Synced);
    assert_eq!(rig.target.state().applies, 1);
    assert_eq!(rig.target.state().governed, 1);
    assert_eq!(rig.poll(Trigger::Timer).await, Outcome::Synced);
    assert_eq!(rig.target.state().applies, 1);
    let resigned = manifest(json!({ "manifest_version": version(0, "0000000000000002") }));
    rig.serve(Ok(fetched(resigned, "e2")));
    assert_eq!(rig.poll(Trigger::Timer).await, Outcome::Synced);
    assert_eq!(rig.target.state().applies, 1);
    let applied = rig.store.state().applied.clone();
    assert_eq!(
        applied,
        Some(ManifestVersion::try_new(version(0, "0000000000000002")).unwrap())
    );
    let calls = rig.store.instance.state().manifest_calls.clone();
    assert_eq!(calls[1].0.as_deref(), Some("e1"));
    assert_eq!(calls[2].0.as_deref(), Some("e1"));
    assert!(
        matches!(&*rig.status.borrow(), SyncStatus::Synced { counts, .. } if counts.skills == 1)
    );
}

#[tokio::test]
async fn another_users_manifest_is_refused_and_its_etag_not_kept() {
    let mut rig = rig();
    rig.serve(Ok(fetched(manifest(json!({ "user_id": "u-2" })), "e9")));
    let outcome = rig.poll(Trigger::Start).await;
    assert_eq!(
        outcome,
        Outcome::Failed {
            kind: SyncErrorKind::Invalid,
            retry_after: None
        }
    );
    assert_eq!(rig.target.state().applies, 0);
    rig.poll(Trigger::Timer).await;
    assert_eq!(rig.store.instance.state().manifest_calls[1].0, None);
    assert!(matches!(
        &*rig.status.borrow(),
        SyncStatus::Error {
            kind: SyncErrorKind::Invalid,
            ..
        }
    ));
}

#[tokio::test]
async fn a_manifest_not_newer_than_the_persisted_version_is_refused() {
    let mut rig = rig();
    rig.store.state().applied =
        Some(ManifestVersion::try_new(version(0, "0000000000000005")).unwrap());
    rig.serve(Ok(fetched(manifest(json!({})), "e1")));
    let outcome = rig.poll(Trigger::Start).await;
    assert_eq!(
        outcome,
        Outcome::Failed {
            kind: SyncErrorKind::Invalid,
            retry_after: None
        }
    );
    assert_eq!(rig.target.state().applies, 0);
}

#[tokio::test]
async fn without_a_pinned_key_the_served_key_is_offered_and_the_loop_halts() {
    let mut rig = rig();
    rig.store.state().pinned = None;
    rig.store.instance.state().served_key = Some(key());
    assert_eq!(rig.poll(Trigger::Start).await, Outcome::Halted);
    assert_eq!(rig.store.state().offered, Some(key()));
    assert_eq!(
        rig.store.instance.state().manifest_calls,
        Vec::<(Option<String>, bool)>::new()
    );
    assert!(matches!(
        &*rig.status.borrow(),
        SyncStatus::Error {
            kind: SyncErrorKind::KeyChanged,
            ..
        }
    ));
}

#[tokio::test]
async fn a_rotated_key_is_offered_and_halts_the_loop() {
    let mut rig = rig();
    rig.store.instance.state().served_key = Some(
        systemprompt_device::client::ManifestKey::from_raw([9u8; 32]),
    );
    let changed = systemprompt_device::client::ClientError::KeyChanged {
        pinned: key().fingerprint().clone(),
        offered: systemprompt_device::client::ManifestKey::from_raw([9u8; 32])
            .fingerprint()
            .clone(),
    };
    rig.serve(Err(changed));
    assert_eq!(rig.poll(Trigger::Start).await, Outcome::Halted);
    assert_eq!(
        rig.store.state().offered,
        Some(systemprompt_device::client::ManifestKey::from_raw(
            [9u8; 32]
        ))
    );
}

#[tokio::test]
async fn an_unreachable_instance_is_a_status_with_its_kind() {
    let mut rig = rig();
    rig.serve(Err(unreachable()));
    let outcome = rig.poll(Trigger::Start).await;
    assert_eq!(
        outcome,
        Outcome::Failed {
            kind: SyncErrorKind::Unreachable,
            retry_after: None
        }
    );
    assert!(matches!(
        &*rig.status.borrow(),
        SyncStatus::Error {
            kind: SyncErrorKind::Unreachable,
            ..
        }
    ));
}

#[tokio::test]
async fn a_failed_apply_is_a_service_error_and_the_next_poll_applies() {
    let mut rig = rig();
    rig.target.state().failing_applies = 1;
    rig.serve(Ok(fetched(manifest(json!({})), "e1")));
    let outcome = rig.poll(Trigger::Start).await;
    assert_eq!(
        outcome,
        Outcome::Failed {
            kind: SyncErrorKind::Service,
            retry_after: None
        }
    );
    assert_eq!(rig.store.state().applied, None);
    assert_eq!(rig.poll(Trigger::Timer).await, Outcome::Synced);
    assert_eq!(rig.target.state().applies, 1);
    assert!(rig.store.state().applied.is_some());
}

#[tokio::test]
async fn only_sync_now_sends_no_cache_and_the_catalog_is_not_read_on_every_poll() {
    let mut rig = rig();
    rig.serve(Ok(fetched(manifest(json!({})), "e1")));
    rig.poll(Trigger::Start).await;
    rig.poll(Trigger::Timer).await;
    rig.poll(Trigger::Focus).await;
    rig.poll(Trigger::SyncNow).await;
    let calls = rig.store.instance.state().manifest_calls.clone();
    let fresh: Vec<bool> = calls.iter().map(|(_, no_cache)| *no_cache).collect();
    assert_eq!(fresh, vec![false, false, false, true]);
    assert_eq!(rig.store.instance.state().catalog_reads, 3);
}

#[tokio::test]
async fn governance_hears_the_extensions_the_manifest_enables_for_this_host() {
    let mut rig = rig();
    rig.serve(Ok(fetched(manifest(json!({})), "e1")));
    assert_eq!(rig.poll(Trigger::Start).await, Outcome::Synced);
    let listed = manifest(json!({
        "manifest_version": version(0, "0000000000000002"),
        "host_extensions": { "harness": ["drive"], "claude-code": ["brain"] },
    }));
    rig.serve(Ok(fetched(listed, "e2")));
    assert_eq!(rig.poll(Trigger::Timer).await, Outcome::Synced);
    let unlisted = manifest(json!({
        "manifest_version": version(0, "0000000000000003"),
        "enabled_hosts": [],
        "host_extensions": { "harness": ["drive"] },
    }));
    rig.serve(Ok(fetched(unlisted, "e3")));
    assert_eq!(rig.poll(Trigger::Timer).await, Outcome::Synced);

    let enabled = rig.target.state().enabled.clone();
    assert_eq!(enabled.len(), 3);
    assert_eq!(enabled[0], None);
    let drive = ExtensionId::parse("drive").unwrap();
    let listed = enabled[1].as_ref().unwrap();
    assert_eq!(listed.iter().collect::<Vec<_>>(), [&drive]);
    assert_eq!(enabled[2], None);
}
