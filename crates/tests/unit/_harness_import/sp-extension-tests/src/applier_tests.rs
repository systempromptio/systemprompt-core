//! Appliers follow the applied instance and withhold what it did not enable.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use sp_client::{CredentialError, CredentialSource, Freshness, HttpClient, SpClient};
use sp_extension::{
    AppliedInstance, ApplyError, EnabledExtensions, Enablement, ExtensionError, ExtensionMetadata,
    ManifestApplier, ManifestAppliers, Resolved,
};
use sp_projection::{Applied, Counts, Gateway};
use sp_protocol::Jwt;
use sp_protocol::ids::{AccountId, InstanceKey};

use crate::support::id;

const INSTANCE: &str = "https://sp.example.com";

struct NoCredentials;

#[async_trait::async_trait]
impl CredentialSource for NoCredentials {
    async fn jwt(&self, _freshness: Freshness) -> Result<Jwt, CredentialError> {
        Err(CredentialError::SignedOut)
    }

    async fn rejected(&self) {}
}

#[derive(Default)]
struct Recorder {
    applied: AtomicUsize,
    cleared: AtomicUsize,
    fails: bool,
}

impl ManifestApplier for Recorder {
    fn apply(&self, _instance: &AppliedInstance) -> Result<(), ApplyError> {
        self.applied.fetch_add(1, Ordering::SeqCst);
        if self.fails {
            return Err(ApplyError::new(CredentialError::Refused));
        }
        Ok(())
    }

    fn clear(&self) {
        self.cleared.fetch_add(1, Ordering::SeqCst);
    }
}

fn instance(extensions: Option<EnabledExtensions>) -> AppliedInstance {
    AppliedInstance {
        account: AccountId::new("account-1"),
        applied: Applied {
            instance_url: INSTANCE.to_owned(),
            instance_key: InstanceKey::new("alpha"),
            gateway: Gateway::default(),
            mcp: Vec::new(),
            governance_plugin: None,
            skill_owners: std::collections::BTreeMap::new(),
            publications: Vec::new(),
            default_theme: None,
            counts: Counts::default(),
        },
        client: SpClient::new(INSTANCE, HttpClient::new("harness-tests/0").unwrap()).unwrap(),
        credentials: Arc::new(NoCredentials),
        organisation: None,
        extensions,
    }
}

struct Rig {
    governance: Arc<Recorder>,
    drive: Arc<Recorder>,
    brain: Arc<Recorder>,
    enablement: Enablement,
    appliers: ManifestAppliers,
}

fn entry(
    metadata: ExtensionMetadata,
    applier: &Arc<Recorder>,
) -> Resolved<Vec<Arc<dyn ManifestApplier>>> {
    let applier: Arc<dyn ManifestApplier> = Arc::<Recorder>::clone(applier);
    Resolved {
        metadata,
        value: vec![applier],
    }
}

fn rig(brain_fails: bool) -> Rig {
    let governance = Arc::new(Recorder::default());
    let drive = Arc::new(Recorder::default());
    let brain = Arc::new(Recorder {
        fails: brain_fails,
        ..Recorder::default()
    });
    let enablement = Enablement::new();
    let appliers = ManifestAppliers::new(
        vec![
            entry(
                ExtensionMetadata::new(id("governance"), "g").mark_required(),
                &governance,
            ),
            entry(ExtensionMetadata::new(id("drive"), "d"), &drive),
            entry(ExtensionMetadata::new(id("brain"), "b"), &brain),
        ],
        enablement.clone(),
    );
    Rig {
        governance,
        drive,
        brain,
        enablement,
        appliers,
    }
}

fn counts(recorder: &Recorder) -> (usize, usize) {
    (
        recorder.applied.load(Ordering::SeqCst),
        recorder.cleared.load(Ordering::SeqCst),
    )
}

#[test]
fn an_instance_without_enablement_reaches_only_required_appliers() {
    let rig = rig(false);
    rig.appliers.apply(&instance(None)).unwrap();
    assert_eq!(counts(&rig.governance), (1, 0));
    assert_eq!(counts(&rig.drive), (0, 1));
    assert_eq!(counts(&rig.brain), (0, 1));
    assert_eq!(rig.enablement.current(), None);
}

#[test]
fn enabled_extensions_apply_and_the_enablement_is_published() {
    let rig = rig(false);
    let enabled: EnabledExtensions = std::iter::once(id("drive")).collect();
    rig.appliers
        .apply(&instance(Some(enabled.clone())))
        .unwrap();
    assert_eq!(counts(&rig.drive), (1, 0));
    assert_eq!(counts(&rig.brain), (0, 1));
    assert_eq!(rig.enablement.current(), Some(enabled));
}

#[test]
fn a_failing_applier_is_reported_and_the_others_still_apply() {
    let rig = rig(true);
    let enabled: EnabledExtensions = [id("drive"), id("brain")].into_iter().collect();
    let error = rig.appliers.apply(&instance(Some(enabled))).unwrap_err();
    assert!(
        matches!(error, ExtensionError::Apply { ref extension, .. } if *extension == id("brain"))
    );
    assert_eq!(counts(&rig.governance), (1, 0));
    assert_eq!(counts(&rig.drive), (1, 0));
}

#[test]
fn clear_reaches_every_applier_and_withdraws_the_enablement() {
    let rig = rig(false);
    let enabled: EnabledExtensions = std::iter::once(id("drive")).collect();
    rig.appliers.apply(&instance(Some(enabled))).unwrap();
    rig.appliers.clear();
    assert_eq!(counts(&rig.governance), (1, 1));
    assert_eq!(counts(&rig.drive), (1, 1));
    assert_eq!(counts(&rig.brain), (0, 2));
    assert_eq!(rig.enablement.current(), None);
}
