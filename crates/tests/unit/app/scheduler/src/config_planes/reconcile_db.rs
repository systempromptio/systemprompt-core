use std::sync::Arc;

use systemprompt_provider_contracts::{
    ApplyMode, ChangeActor, ChangeRequest, ConfigPlaneError, Dependencies, EntityScope,
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

#[tokio::test]
async fn file_authority_reports_drift_and_writes_nothing() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("rec_file"),
        &[("a", "console-edit"), ("b", "2"), ("stray", "x")],
    ));
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2"), ("c", "3")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Scheduled).await;
    let row = report
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane");

    assert_eq!(row.outcome, PlaneOutcome::DriftReported);
    assert_eq!(row.authority, EffectiveAuthority::File);
    assert_eq!(row.drift_rows, 3);
    let store = plane.store();
    assert_eq!(store.get("a").map(String::as_str), Some("console-edit"));
    assert!(store.contains_key("stray"));
    assert!(plane.applied_reasons.lock().expect("lock").is_empty());
    assert!(
        svc.repository()
            .list_changes(plane.name, 10)
            .await
            .expect("audit")
            .is_empty()
    );

    let state = svc.repository().list_states().await.expect("states");
    let state = state.iter().find(|s| s.plane == plane.name).expect("state");
    assert_eq!(
        (state.outcome.as_str(), state.drift_rows),
        ("drift_reported", 3)
    );
    assert_eq!(state.declared_hash, row.declared_hash);
}

#[tokio::test]
async fn db_authority_reports_drift_without_writing() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("rec_db"),
        &[("a", "console")],
    ));
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Scheduled).await;
    let row = report
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane");

    assert_eq!(row.outcome, PlaneOutcome::DriftReported);
    assert_eq!(row.drift_rows, 2);
    assert_eq!(plane.store().get("a").map(String::as_str), Some("console"));
    assert!(plane.applied_reasons.lock().expect("lock").is_empty());
    assert!(
        svc.repository()
            .list_changes(plane.name, 10)
            .await
            .expect("audit")
            .is_empty()
    );
}

#[tokio::test]
async fn boot_never_seeds_an_empty_projection() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("rec_seed"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Boot).await;
    let row = report
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane");

    assert_eq!(row.outcome, PlaneOutcome::DriftReported);
    assert!(plane.store().is_empty());
    assert!(
        svc.repository()
            .list_changes(plane.name, 10)
            .await
            .expect("audit")
            .is_empty()
    );
}

#[tokio::test]
async fn a_missing_declaration_is_undeclared_and_never_written() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("rec_absent"),
        &[("a", "1")],
    ));
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Scheduled).await;
    let row = report
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane");

    assert_eq!(row.outcome, PlaneOutcome::Undeclared);
    assert_eq!(row.authority, EffectiveAuthority::Undeclared);
    assert_eq!(plane.store().len(), 1);
}

#[tokio::test]
async fn an_operator_apply_that_does_not_converge_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("rec_stuck"), &[("a", "x")]).stuck());
    plane.write_declaration(root.path(), &[("a", "1")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;
    let change = ChangeRequest {
        actor: ChangeActor::Cli,
        reason: "stuck",
    };

    let err = svc
        .apply(plane.name, ApplyMode::Overwrite, &change, EntityScope::All)
        .await
        .expect_err("not converged");

    assert!(err.to_string().contains("did not converge"), "{err}");
    assert!(
        svc.repository()
            .list_changes(plane.name, 10)
            .await
            .expect("audit")
            .is_empty(),
        "a rolled-back apply leaves no audit rows"
    );
}

#[tokio::test]
async fn an_unreadable_declaration_fails_the_plane() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("rec_bad"),
        &[("live", "row")],
    ));
    let path = root.path().join(plane.path);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "no separator here\n").expect("write");
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Boot).await;

    assert_eq!(report.failures().len(), 1);
    assert_eq!(
        plane.store().get("live").map(String::as_str),
        Some("row"),
        "a declaration that cannot be parsed must never prune the live projection"
    );
}

#[tokio::test]
async fn a_declaration_that_cannot_be_read_never_deletes_live_rows() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("rec_io"),
        &[("live", "row"), ("other", "x")],
    ));
    std::fs::create_dir_all(root.path().join(plane.path)).expect("directory in place of file");
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Boot).await;

    assert_eq!(report.failures().len(), 1);
    assert_eq!(plane.store().len(), 2);
    assert!(
        svc.repository()
            .list_changes(plane.name, 10)
            .await
            .expect("audit")
            .is_empty()
    );
}

#[tokio::test]
async fn operator_diff_and_insert_only_apply_record_the_reason() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("rec_cli"),
        &[("a", "console"), ("keep", "k")],
    ));
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name])).await;

    let diff = svc.diff(plane.name).await.expect("diff");
    assert_eq!(diff.rows.len(), 3);
    assert_eq!(diff.authority, EffectiveAuthority::Db);

    let change = ChangeRequest {
        actor: ChangeActor::Cli,
        reason: "ticket OPS-1",
    };
    let applied = svc
        .apply(plane.name, ApplyMode::InsertOnly, &change, EntityScope::All)
        .await
        .expect("apply");

    assert_eq!(applied.changes, 1);
    assert_eq!(applied.remaining_drift, 2);
    assert_eq!(plane.store().get("a").map(String::as_str), Some("console"));
    let audit = svc
        .repository()
        .list_changes(plane.name, 10)
        .await
        .expect("audit");
    assert_eq!(audit.len(), 1);
    assert_eq!(
        (
            audit[0].entity.as_str(),
            audit[0].actor.as_str(),
            audit[0].reason.as_str()
        ),
        ("b", "cli", "ticket OPS-1")
    );
}

#[tokio::test]
async fn an_empty_declaration_reports_every_row_and_deletes_none() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("rec_empty"),
        &[("a", "1"), ("b", "2")],
    ));
    plane.write_declaration(root.path(), &[]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Scheduled).await;
    let row = report
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane");

    assert_eq!(
        (row.outcome, row.drift_rows),
        (PlaneOutcome::DriftReported, 2)
    );
    assert_eq!(plane.store().len(), 2);
    assert!(plane.applied_reasons.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn exclusive_reconcile_takes_and_releases_the_cluster_lock() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("rec_lock"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let wait = std::time::Duration::from_secs(30);
    svc.reconcile_exclusive(ReconcileTrigger::Boot, wait)
        .await
        .expect("first pass");
    svc.reconcile_exclusive(ReconcileTrigger::Boot, wait)
        .await
        .expect("lock released after the first pass");
    assert!(plane.store().is_empty(), "the pass writes nothing");
}

#[tokio::test]
async fn seed_if_empty_from_the_cli_leaves_a_populated_plane_alone() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(
        unique_plane_name("rec_seed_cli"),
        &[("a", "console")],
    ));
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name])).await;
    let change = ChangeRequest {
        actor: ChangeActor::Cli,
        reason: "seed attempt",
    };

    let applied = svc
        .apply(
            plane.name,
            ApplyMode::SeedIfEmpty,
            &change,
            EntityScope::All,
        )
        .await
        .expect("apply");

    assert_eq!((applied.changes, applied.remaining_drift), (0, 2));
    assert!(plane.applied_reasons.lock().expect("lock").is_empty());
    assert_eq!(plane.store().get("a").map(String::as_str), Some("console"));
}

#[tokio::test]
async fn a_scoped_apply_to_a_plane_without_scope_support_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("rec_scope"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name])).await;
    let change = ChangeRequest {
        actor: ChangeActor::Cli,
        reason: "scoped",
    };
    let keys = vec!["a".to_owned()];

    let err = svc
        .apply(
            plane.name,
            ApplyMode::Overwrite,
            &change,
            EntityScope::Entities(&keys),
        )
        .await
        .expect_err("refused");

    assert!(matches!(
        err,
        SchedulerError::ConfigPlane(ConfigPlaneError::Unsupported { .. })
    ));
    assert!(plane.store().is_empty());
}
