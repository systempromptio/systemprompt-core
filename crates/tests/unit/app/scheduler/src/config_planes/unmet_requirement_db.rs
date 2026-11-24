use std::sync::Arc;

use systemprompt_provider_contracts::{
    ApplyMode, ChangeActor, ChangeRequest, ConfigPlaneError, Dependencies, EntityScope,
    StateRequirement,
};
use systemprompt_runtime::ConfigPlaneSet;
use systemprompt_scheduler::SchedulerError;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{
    ConfigReconcileService, PlaneFailure, PlaneOutcome, ReconcileTrigger,
};
use systemprompt_test_fixtures::test_db_pool;

use super::fake_plane::{FakePlane, unique_plane_name};
use super::plane_set;

struct AbsentState;

async fn service(set: ConfigPlaneSet) -> ConfigReconcileService {
    let pool = test_db_pool().await;
    ConfigReconcileService::new(
        set,
        ConfigReconcileRepository::new(&pool),
        Dependencies::new(),
    )
}

fn requiring_absent_state(prefix: &str) -> Arc<FakePlane> {
    Arc::new(
        FakePlane::new(unique_plane_name(prefix), &[("a", "1")])
            .requiring(StateRequirement::of::<AbsentState>()),
    )
}

#[tokio::test]
async fn a_plane_with_an_unmet_requirement_is_unavailable_and_unread() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = requiring_absent_state("unmet_rec");
    plane.write_declaration(root.path(), &[("a", "2")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Scheduled).await;
    let row = report
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane");

    assert_eq!(row.outcome, PlaneOutcome::Unavailable);
    assert!(matches!(
        row.failure,
        Some(PlaneFailure::UnmetRequirement { requirement }) if requirement.ends_with("AbsentState")
    ));
    assert_eq!(row.declared_hash, None, "the declaration is not read");
    assert!(report.failures().is_empty(), "an unavailable plane does not fail the pass");

    let states = svc.repository().list_states().await.expect("states");
    let state = states.iter().find(|s| s.plane == plane.name).expect("state");
    assert_eq!(state.outcome.as_str(), "unavailable");
}

#[tokio::test]
async fn an_apply_to_a_plane_with_an_unmet_requirement_is_refused_typed() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = requiring_absent_state("unmet_apply");
    plane.write_declaration(root.path(), &[("a", "2")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name])).await;
    let change = ChangeRequest {
        actor: ChangeActor::Cli,
        reason: "unmet requirement",
    };

    let refused = svc
        .apply(plane.name, ApplyMode::Overwrite, &change, EntityScope::All)
        .await
        .expect_err("an unmet requirement refuses the apply");

    assert!(matches!(
        refused,
        SchedulerError::ConfigPlane(ConfigPlaneError::UnmetRequirement { ref plane, .. })
            if plane == "unmet_apply" || plane.starts_with("unmet_apply")
    ));
    assert_eq!(plane.store().get("a").map(String::as_str), Some("1"));
    assert!(plane.applied_reasons.lock().expect("lock").is_empty());
}
