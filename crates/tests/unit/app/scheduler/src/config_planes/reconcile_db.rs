use std::sync::Arc;

use systemprompt_provider_contracts::{ApplyMode, ChangeActor, ChangeRequest, Dependencies};
use systemprompt_runtime::{ConfigPlaneSet, EffectiveAuthority};
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
async fn file_authority_overwrites_drift_and_audits_before_and_after() {
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

    assert_eq!(row.outcome, PlaneOutcome::Corrected);
    assert_eq!(row.authority, EffectiveAuthority::File);
    assert_eq!((row.changes, row.drift_rows), (3, 0));
    let store = plane.store();
    assert_eq!(store.get("a").map(String::as_str), Some("1"));
    assert_eq!(store.get("c").map(String::as_str), Some("3"));
    assert!(!store.contains_key("stray"));

    let audit = svc
        .repository()
        .list_changes(plane.name, 10)
        .await
        .expect("audit");
    assert_eq!(audit.len(), 3);
    let a = audit.iter().find(|r| r.entity == "a").expect("a");
    assert_eq!(
        (
            a.change_kind.as_str(),
            a.before_value.as_deref(),
            a.after_value.as_deref()
        ),
        ("changed", Some("console-edit"), Some("1"))
    );
    assert_eq!(a.actor, "config_reconcile");
    assert_eq!(a.mode, "overwrite");
    let stray = audit.iter().find(|r| r.entity == "stray").expect("stray");
    assert_eq!(
        (stray.before_value.as_deref(), stray.after_value.as_deref()),
        (Some("x"), None)
    );

    let state = svc.repository().list_states().await.expect("states");
    let state = state.iter().find(|s| s.plane == plane.name).expect("state");
    assert_eq!((state.outcome.as_str(), state.drift_rows), ("corrected", 0));
    assert_eq!(state.declared_hash, row.declared_hash);
    assert!(state.corrected_at.is_some());

    let again = svc.reconcile_all(ReconcileTrigger::Scheduled).await;
    let again = again
        .planes
        .iter()
        .find(|p| p.plane == plane.name)
        .expect("plane");
    assert_eq!((again.outcome, again.changes), (PlaneOutcome::InSync, 0));
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
    assert_eq!((row.drift_rows, row.changes), (2, 0));
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
async fn db_authority_seeds_an_empty_projection_once() {
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

    assert_eq!(row.outcome, PlaneOutcome::Seeded);
    assert_eq!(plane.store().get("a").map(String::as_str), Some("1"));
    let audit = svc
        .repository()
        .list_changes(plane.name, 10)
        .await
        .expect("audit");
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].actor, "boot");
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
async fn an_apply_that_does_not_converge_fails_the_plane() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("rec_stuck"), &[("a", "x")]).stuck());
    plane.write_declaration(root.path(), &[("a", "1")]);
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Scheduled).await;

    assert_eq!(report.failures().len(), 1);
    let row = &report.failures()[0];
    assert!(
        row.error
            .as_deref()
            .unwrap_or_default()
            .contains("did not converge")
    );
    let state = svc.repository().list_states().await.expect("states");
    assert!(
        state
            .iter()
            .any(|s| s.plane == plane.name && s.outcome == "failed")
    );
}

#[tokio::test]
async fn an_unreadable_declaration_fails_the_plane() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("rec_bad"), &[]));
    let path = root.path().join(plane.path);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "no separator here\n").expect("write");
    let svc = service(plane_set(root.path(), &[Arc::clone(&plane)], &[])).await;

    let report = svc.reconcile_all(ReconcileTrigger::Boot).await;

    assert_eq!(report.failures().len(), 1);
    assert!(plane.store().is_empty());
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
        .apply(plane.name, ApplyMode::InsertOnly, &change)
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
async fn empty_file_declaration_never_wipes_rows_unattended() {
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

    assert_eq!(row.outcome, PlaneOutcome::Failed);
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
    assert_eq!(plane.store().get("a").map(String::as_str), Some("1"));
}
