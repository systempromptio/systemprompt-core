//! The governance extension: required, its applier following the applied
//! instance whatever the enablement, and the Governor serving that enablement.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use harness_engine::Governor;
use harness_tasks::CancellationToken;
use sp_client::{CredentialSource, HttpClient, SpClient};
use sp_extension::{
    AppliedInstance, ApplyError, EnabledExtensions, Enablement, ExtensionError, ExtensionId,
    ExtensionMetadata, ManifestApplier, ManifestAppliers, Resolved, SpExtension,
};
use sp_governance::{Governance, GovernanceApplier, GovernanceExtension};
use sp_protocol::ids::AccountId;

use crate::fake_api::FakeCredentials;
use crate::support::{INSTANCE, applied, rig};

#[derive(Default)]
struct Counting {
    applied: AtomicUsize,
    cleared: AtomicUsize,
}

impl ManifestApplier for Counting {
    fn apply(&self, _instance: &AppliedInstance) -> Result<(), ApplyError> {
        self.applied.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn clear(&self) {
        self.cleared.fetch_add(1, Ordering::SeqCst);
    }
}

fn drive_id() -> ExtensionId {
    ExtensionId::parse("drive").unwrap()
}

fn enabling(ids: &[ExtensionId]) -> EnabledExtensions {
    ids.iter().cloned().collect()
}

fn instance(url: &str, extensions: Option<EnabledExtensions>) -> AppliedInstance {
    let http = HttpClient::new("harness-tests/0").unwrap();
    let credentials: Arc<dyn CredentialSource> = Arc::new(FakeCredentials::default());
    let mut snapshot = applied(true);
    snapshot.instance_url = url.to_owned();
    AppliedInstance {
        account: AccountId::new("a-1"),
        applied: snapshot,
        client: SpClient::new(INSTANCE, http).unwrap(),
        credentials,
        organisation: Some("Alpha Org".to_owned()),
        extensions,
    }
}

struct Appliers {
    governance: Arc<Governance>,
    drive: Arc<Counting>,
    enablement: Enablement,
    set: ManifestAppliers,
}

fn appliers() -> Appliers {
    let governance = Governance::new(&CancellationToken::new());
    let drive = Arc::new(Counting::default());
    let enablement = Enablement::new();
    let governed: Arc<dyn ManifestApplier> =
        Arc::new(GovernanceApplier::new(Arc::clone(&governance)));
    let counting: Arc<dyn ManifestApplier> = Arc::<Counting>::clone(&drive);
    let set = ManifestAppliers::new(
        vec![
            Resolved {
                metadata: GovernanceExtension.metadata(),
                value: vec![governed],
            },
            Resolved {
                metadata: ExtensionMetadata::new(drive_id(), "Drive"),
                value: vec![counting],
            },
        ],
        enablement.clone(),
    );
    Appliers {
        governance,
        drive,
        enablement,
        set,
    }
}

#[test]
fn the_governance_extension_is_required_under_its_own_id() {
    let metadata = GovernanceExtension.metadata();
    assert_eq!(metadata.id.as_str(), "governance");
    assert_eq!(metadata.id, GovernanceExtension::ID);
    assert!(metadata.required);
    assert_eq!(metadata.dependencies, Vec::<ExtensionId>::new());
}

#[tokio::test]
async fn no_instance_and_no_named_extension_both_withhold() {
    let governance = Governance::new(&CancellationToken::new());
    assert_eq!(governance.extensions(), None);

    let rig = rig(true, 3_600);
    assert_eq!(rig.governance.extensions(), None);

    let mut parts = rig.parts(true, None);
    parts.extensions = Some(enabling(&[drive_id()]));
    rig.governance.install(parts).unwrap();
    let served = rig.governance.extensions().unwrap();
    assert!(served.contains(&drive_id()));
    assert!(!served.contains(&GovernanceExtension::ID));

    rig.governance.clear();
    assert_eq!(rig.governance.extensions(), None);
}

#[tokio::test]
async fn governance_takes_the_instance_even_when_nothing_is_enabled() {
    let rig = appliers();
    rig.set.apply(&instance(INSTANCE, None)).unwrap();
    assert!(rig.governance.applied().is_some());
    assert_eq!(rig.governance.governed_by().as_deref(), Some("Alpha Org"));
    assert_eq!(rig.governance.extensions(), None);
    assert_eq!(rig.enablement.current(), None);
    assert_eq!(rig.drive.applied.load(Ordering::SeqCst), 0);
    assert_eq!(rig.drive.cleared.load(Ordering::SeqCst), 1);
    assert_eq!(rig.governance.models().len(), 2);
}

#[tokio::test]
async fn an_enabled_extension_is_applied_beside_governance_and_cleared_with_it() {
    let rig = appliers();
    rig.set
        .apply(&instance(INSTANCE, Some(enabling(&[drive_id()]))))
        .unwrap();
    assert!(rig.governance.applied().is_some());
    assert_eq!(rig.governance.extensions(), Some(enabling(&[drive_id()])));
    assert_eq!(rig.enablement.current(), Some(enabling(&[drive_id()])));
    assert_eq!(rig.drive.applied.load(Ordering::SeqCst), 1);

    rig.set.clear();
    assert!(rig.governance.applied().is_none());
    assert_eq!(rig.governance.models().len(), 0);
    assert_eq!(rig.governance.extensions(), None);
    assert_eq!(rig.enablement.current(), None);
    assert_eq!(rig.drive.cleared.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_instance_its_client_does_not_reach_leaves_governance_empty() {
    let rig = appliers();
    let error = rig
        .set
        .apply(&instance(
            "https://elsewhere.example.com",
            Some(enabling(&[drive_id()])),
        ))
        .unwrap_err();
    let ExtensionError::Apply { extension, .. } = error else {
        panic!("expected an apply failure, got {error:?}");
    };
    assert_eq!(extension, GovernanceExtension::ID);
    assert!(rig.governance.applied().is_none());
    assert_eq!(rig.governance.extensions(), None);
}
