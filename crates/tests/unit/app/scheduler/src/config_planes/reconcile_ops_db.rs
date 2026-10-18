use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_provider_contracts::{
    ApplyMode, ChangeActor, ChangeRequest, ConfigPlane, ConfigPlaneError, Dependencies,
    EntityScope, PlaneAuthority,
};
use systemprompt_runtime::{ConfigPlaneSet, EffectiveAuthority};
use systemprompt_scheduler::SchedulerError;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{ConfigReconcileService, PlaneOutcome, ReconcileTrigger};
use systemprompt_test_fixtures::test_db_pool;

use super::fake_plane::{FakePlane, unique_plane_name};
use super::plane_set;

async fn service(set: ConfigPlaneSet) -> ConfigReconcileService {
    let pool = test_db_pool().await;
    ConfigReconcileService::new(
        set,
        ConfigReconcileRepository::new(&pool),
        Dependencies::new(),
    )
}

fn change() -> ChangeRequest<'static> {
    ChangeRequest {
        actor: ChangeActor::Cli,
        reason: "operator apply",
    }
}

#[tokio::test]
async fn export_of_a_plane_without_export_support_is_unsupported() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("ops_export"), &[]));
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let err = svc.export(plane.name).await.expect_err("unsupported");
    assert!(
        matches!(
            err,
            SchedulerError::ConfigPlane(ConfigPlaneError::Unsupported { .. })
        ),
        "got {err:?}"
    );
}

#[tokio::test]
async fn operations_on_an_unknown_plane_are_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let svc = service(plane_set(root.path(), &[], &[])).await;

    let err = svc.diff("no_such_plane").await.expect_err("unknown");
    assert!(matches!(
        err,
        SchedulerError::ConfigPlane(ConfigPlaneError::UnknownPlane(_))
    ));
    let err = svc
        .apply(
            "no_such_plane",
            ApplyMode::Overwrite,
            &change(),
            EntityScope::All,
        )
        .await
        .expect_err("unknown");
    assert!(matches!(
        err,
        SchedulerError::ConfigPlane(ConfigPlaneError::UnknownPlane(_))
    ));
}

#[tokio::test]
async fn diff_of_an_undeclared_plane_has_no_rows_and_no_hash() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("ops_diff_undecl"),
        &[("a", "1")],
    ));
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let diff = svc.diff(plane.name).await.expect("diff");
    assert!(diff.rows.is_empty());
    assert!(diff.declared_hash.is_none());
    assert_eq!(diff.plane, plane.name);
    assert_eq!(diff.declared_path, plane.path);
}

#[tokio::test]
async fn applying_a_plane_whose_declaration_is_missing_is_invalid() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("ops_apply_missing"),
        &[("a", "1")],
    ));
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let err = svc
        .apply(
            plane.name,
            ApplyMode::Overwrite,
            &change(),
            EntityScope::All,
        )
        .await
        .expect_err("no declaration");
    assert!(matches!(
        err,
        SchedulerError::ConfigPlane(ConfigPlaneError::Invalid { .. })
    ));
    assert_eq!(plane.store().len(), 1, "nothing is written");
    assert!(plane.applied_reasons.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn an_overwrite_apply_converges_and_audits_each_change() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("ops_apply_ok"),
        &[("a", "old"), ("gone", "x")],
    ));
    plane.write_declaration(root.path(), &[("a", "new"), ("b", "2")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name])).await;

    let applied = svc
        .apply(
            plane.name,
            ApplyMode::Overwrite,
            &change(),
            EntityScope::All,
        )
        .await
        .expect("apply");

    assert_eq!((applied.changes, applied.remaining_drift), (3, 0));
    assert_eq!(plane.store().get("a").map(String::as_str), Some("new"));
    assert!(!plane.store().contains_key("gone"));
    let audit = svc
        .repository()
        .list_changes(plane.name, 10)
        .await
        .expect("audit");
    assert_eq!(audit.len(), 3);
    assert!(audit.iter().all(|r| r.reason == "operator apply"));
}

#[tokio::test]
async fn a_moved_declaration_is_corrected_again_and_the_state_follows_its_hash() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("ops_rehash"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let first = svc.reconcile_all(ReconcileTrigger::Boot).await;
    let first = first
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane")
        .clone();
    assert_eq!(first.outcome, PlaneOutcome::Corrected);

    plane.write_declaration(root.path(), &[("a", "2")]);
    let second = svc.reconcile_all(ReconcileTrigger::Scheduled).await;
    let second = second
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane")
        .clone();
    assert_eq!(second.outcome, PlaneOutcome::Corrected);
    assert_ne!(first.declared_hash, second.declared_hash);

    let states = svc.repository().list_states().await.expect("states");
    let state = states
        .iter()
        .find(|s| s.plane == plane.name)
        .expect("state");
    assert_eq!(state.declared_hash, second.declared_hash);
}

#[tokio::test]
async fn a_plane_moved_from_file_to_db_authority_is_recorded_under_db() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("ops_authority"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);

    let file_svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;
    file_svc.reconcile_all(ReconcileTrigger::Scheduled).await;

    let db_svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name])).await;
    let report = db_svc.reconcile_all(ReconcileTrigger::Scheduled).await;
    let row = report
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane");
    assert_eq!(row.authority, EffectiveAuthority::Db);
    assert_eq!(row.outcome, PlaneOutcome::InSync);

    let states = db_svc.repository().list_states().await.expect("states");
    let state = states
        .iter()
        .find(|s| s.plane == plane.name)
        .expect("state");
    assert_eq!(state.authority, "db");
}

#[tokio::test]
async fn an_authority_key_for_an_unregistered_plane_does_not_stop_the_pass() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("ops_ghost"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);
    let mut authority: BTreeMap<String, PlaneAuthority> = BTreeMap::new();
    authority.insert(
        unique_plane_name("ops_ghost_key").to_owned(),
        PlaneAuthority::Db,
    );
    let planes: Vec<Arc<dyn ConfigPlane>> = vec![Arc::clone(&plane) as Arc<dyn ConfigPlane>];
    let set = ConfigPlaneSet::new(planes, authority, root.path().to_path_buf()).expect("set");
    let svc = service(set).await;

    let report = svc.reconcile_all(ReconcileTrigger::Scheduled).await;
    let row = report
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane");
    assert_eq!(row.outcome, PlaneOutcome::Corrected);
}
