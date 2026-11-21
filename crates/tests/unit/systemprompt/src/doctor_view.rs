//! The view a contributed `cloud doctor` check receives: secret presence and
//! read-only probes only, and the same view for core and install members.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_extension::AdmittedDoctorChecks;
use systemprompt_identifiers::ManagedSourceId;
use systemprompt_provider_contracts::{
    DoctorCheck, DoctorContext, DoctorOutcome, DoctorProbeError, DoctorProbes, DoctorStatus,
    GitCredentialSource, SecretPresence,
};

use crate::probe::{Probe, profile};

const INSTALL: &str = "probe-doctor";

struct FixedProbes;

#[async_trait]
impl DoctorProbes for FixedProbes {
    async fn git_credential_sources(&self) -> Result<Vec<GitCredentialSource>, DoctorProbeError> {
        Ok(vec![GitCredentialSource {
            id: ManagedSourceId::new("source-1"),
            name: "ci".to_owned(),
            repository: "https://example.invalid/ci.git".to_owned(),
            credential_reference: "ci".to_owned(),
        }])
    }
}

#[derive(Default)]
struct Recording {
    seen: Mutex<Option<(bool, usize)>>,
}

#[async_trait]
impl DoctorCheck for Recording {
    fn name(&self) -> &'static str {
        "probe-view"
    }

    async fn check(&self, ctx: DoctorContext<'_>) -> DoctorOutcome {
        let sources = ctx.probes.git_credential_sources().await.map_or(0, |s| s.len());
        *self.seen.lock().expect("lock") = Some((ctx.secrets.is_present("present"), sources));
        DoctorOutcome::pass("seen")
    }
}

fn view_of(ctx: DoctorContext<'_>) -> (&dyn SecretPresence, &dyn DoctorProbes) {
    let DoctorContext { secrets, probes } = ctx;
    (secrets, probes)
}

#[test]
fn a_doctor_context_holds_only_secret_presence_and_read_only_probes() {
    let secrets: HashMap<String, String> =
        HashMap::from([("present".to_owned(), "value".to_owned())]);
    let ctx = DoctorContext {
        secrets: &secrets,
        probes: &FixedProbes,
    };
    let (presence, _probes) = view_of(ctx);
    assert!(presence.is_present("present"));
    assert!(!presence.is_present("absent"));
}

#[test]
fn install_and_core_checks_run_against_the_same_view() {
    let recording = Arc::new(Recording::default());
    let contributed = Arc::clone(&recording);
    let admitted = systemprompt::reference()
        .with(Probe::new(INSTALL, move |c| {
            c.doctor_check(Arc::clone(&contributed) as Arc<dyn DoctorCheck>);
        }))
        .build(&profile(&[]))
        .expect("the composition admits");
    let checks = AdmittedDoctorChecks::of(admitted.built());
    let secrets: HashMap<String, String> =
        HashMap::from([("present".to_owned(), "value".to_owned())]);
    let ctx = DoctorContext {
        secrets: &secrets,
        probes: &FixedProbes,
    };
    let outcomes = futures::executor::block_on(checks.run(ctx));

    let install = outcomes
        .iter()
        .find(|(owned, _)| owned.owner.as_str() == INSTALL)
        .expect("the install member's check runs");
    assert_eq!(install.1.status, DoctorStatus::Pass);
    assert_eq!(*recording.seen.lock().expect("lock"), Some((true, 1)));

    let core = outcomes
        .iter()
        .find(|(owned, _)| owned.check.name() == "git-credentials")
        .expect("the core marketplace check runs");
    assert_ne!(core.0.owner.as_str(), INSTALL);
    assert_eq!(core.1.status, DoctorStatus::Warn);
    assert!(core.1.detail.contains("no credential secret found for: ci"));
}
