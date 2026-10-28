use std::path::Path;
use std::sync::{Arc, Mutex};

use systemprompt_database::DbPool;
use systemprompt_extension::{Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry};
use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_provider_contracts::{
    ApplyMode, ChangeActor, ChangeRequest, ConfigPlane, ConfigPlaneError, EntityScope,
    PlaneAuthority, PlaneEffect,
};
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{
    ApplyOptions, ConfigApplyService, ConfigReconcileService, PlaneOutcome, RECONCILE_LOCK_WAIT,
    ReconcileTrigger,
};
use systemprompt_test_fixtures::{
    fixture_app_context_with_extensions, fixture_config, test_database_url, test_db_pool,
};

use super::db_plane::{ApplyLog, DbPlane, ensure_table, seed_rows, stored_rows};
use super::fake_plane::unique_plane_name;

struct PlanesExtension {
    id: &'static str,
    planes: Vec<Arc<dyn ConfigPlane>>,
}

impl Extension for PlanesExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "Config planes e2e",
            version: "1.0.0",
        }
    }

    fn config_planes(&self) -> Vec<Arc<dyn ConfigPlane>> {
        self.planes.clone()
    }
}

struct Harness {
    pool: DbPool,
    ctx: Arc<AppContext>,
    service: ConfigReconcileService,
}

async fn harness(root: &Path, planes: Vec<Arc<dyn ConfigPlane>>, db: &[&str]) -> Harness {
    let authority: Vec<(&str, PlaneAuthority)> =
        db.iter().map(|name| (*name, PlaneAuthority::Db)).collect();
    harness_with(root, planes, &authority).await
}

async fn harness_with(
    root: &Path,
    planes: Vec<Arc<dyn ConfigPlane>>,
    authority: &[(&str, PlaneAuthority)],
) -> Harness {
    let pool = test_db_pool().await;
    ensure_table(&pool).await;
    let mut config = fixture_config(&test_database_url());
    config.services_path = root.display().to_string();
    for (name, value) in authority {
        config.planes.authority.insert((*name).to_owned(), *value);
    }
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(PlanesExtension {
            id: unique_plane_name("planes_ext"),
            planes,
        }))
        .expect("register extension");
    let ctx = fixture_app_context_with_extensions(&pool, config, registry).expect("context");
    let service =
        ConfigReconcileService::from_app_context(&ctx, ConfigReconcileRepository::new(&pool))
            .expect("service");
    Harness { pool, ctx, service }
}

fn new_log() -> ApplyLog {
    Arc::new(Mutex::new(Vec::new()))
}

fn snapshot(pairs: &[(&str, &str)]) -> systemprompt_provider_contracts::PlaneSnapshot {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[tokio::test]
async fn boot_reports_planes_in_dependency_order_and_config_apply_writes_them() {
    let root = tempfile::tempdir().expect("tempdir");
    let log = new_log();
    let parent = DbPlane::new(unique_plane_name("e2e_parent"), &log);
    let mut child = DbPlane::new(unique_plane_name("e2e_child"), &log);
    child.deps = Box::leak(vec![parent.name].into_boxed_slice());
    parent.write_declaration(root.path(), &[("g", "1")]);
    child.write_declaration(root.path(), &[("rule", "g")]);
    let (parent_name, child_name) = (parent.name, child.name);
    let h = harness(root.path(), vec![Arc::new(child), Arc::new(parent)], &[]).await;

    let report = h
        .service
        .reconcile_exclusive(ReconcileTrigger::Boot, RECONCILE_LOCK_WAIT)
        .await
        .expect("boot pass");

    assert!(report.failures().is_empty(), "{:?}", report.failures());
    let order: Vec<&str> = report.planes.iter().map(|p| p.plane.as_str()).collect();
    assert_eq!(order, vec![parent_name, child_name]);
    assert!(log.lock().expect("lock").is_empty(), "boot writes nothing");
    assert!(stored_rows(&h.pool, parent_name).await.is_empty());

    let apply = ConfigApplyService::new(h.service.clone());
    let planes = vec![parent_name.to_owned(), child_name.to_owned()];
    let merged = apply
        .apply(&ApplyOptions {
            planes: &planes,
            actor: ChangeActor::Cli,
            reason: "deploy",
            dry_run: false,
            wait: RECONCILE_LOCK_WAIT,
        })
        .await
        .expect("config apply");

    assert_eq!(merged.applied(), 2);
    assert_eq!(*log.lock().expect("lock"), vec![parent_name, child_name]);
    assert_eq!(
        stored_rows(&h.pool, child_name).await,
        snapshot(&[("rule", "g")])
    );
    let audit = h
        .service
        .repository()
        .list_changes(child_name, 10)
        .await
        .expect("audit");
    assert_eq!(
        (audit.len(), audit[0].actor.as_str(), audit[0].mode.as_str()),
        (1, "cli", "merge")
    );
}

#[tokio::test]
async fn the_extension_context_writer_refuses_a_file_managed_plane_with_409() {
    let root = tempfile::tempdir().expect("tempdir");
    let log = new_log();
    let file_plane = DbPlane::new(unique_plane_name("e2e_wfile"), &log);
    let db_plane = DbPlane::new(unique_plane_name("e2e_wdb"), &log);
    file_plane.write_declaration(root.path(), &[("a", "1")]);
    db_plane.write_declaration(root.path(), &[("a", "1")]);
    let (file_name, db_name) = (file_plane.name, db_plane.name);
    let h = harness_with(
        root.path(),
        vec![Arc::new(file_plane), Arc::new(db_plane)],
        &[(file_name, PlaneAuthority::File)],
    )
    .await;

    let set = ExtensionContext::config_planes(&*h.ctx).expect("planes");
    let api: ApiError = set.writer(file_name).expect_err("file-managed").into();
    assert!(matches!(api.code, ErrorCode::ConflictError));
    assert_eq!(api.error_key.as_deref(), Some("config_plane_file_managed"));
    assert_eq!(set.writer(db_name).expect("db authority").name(), db_name);
}

#[tokio::test]
async fn export_round_trips_console_rows_into_a_clean_declaration() {
    let root = tempfile::tempdir().expect("tempdir");
    let log = new_log();
    let plane = DbPlane::new(unique_plane_name("e2e_export"), &log);
    let (name, path) = (plane.name, plane.path);
    let h = harness(root.path(), vec![Arc::new(plane)], &[name]).await;
    seed_rows(&h.pool, name, &[("console_a", "x"), ("console_b", "y")]).await;

    let text = h
        .service
        .export(name)
        .await
        .expect("export")
        .expect("rows to export");
    std::fs::create_dir_all(root.path().join(path).parent().expect("parent")).expect("mkdir");
    std::fs::write(root.path().join(path), &text).expect("write export");

    let declared = h
        .service
        .planes()
        .find(name)
        .expect("plane")
        .load_declared(root.path())
        .expect("load")
        .expect("declared");
    assert_eq!(declared.entries, stored_rows(&h.pool, name).await);
    let diff = h.service.diff(name).await.expect("diff");
    assert!(diff.rows.is_empty(), "{:?}", diff.rows);
}

#[tokio::test]
async fn export_of_an_empty_plane_is_none() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = DbPlane::new(unique_plane_name("e2e_export_empty"), &new_log());
    let name = plane.name;
    let h = harness(root.path(), vec![Arc::new(plane)], &[name]).await;

    assert!(h.service.export(name).await.expect("export").is_none());
}

#[tokio::test]
async fn console_edits_survive_boot_passes_and_later_applies() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut plane = DbPlane::new(unique_plane_name("e2e_seed"), &new_log());
    plane.effect = PlaneEffect::OnRestart;
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2")]);
    let name = plane.name;
    let h = harness(root.path(), vec![Arc::new(plane)], &[name]).await;
    let apply = ConfigApplyService::new(h.service.clone());
    let planes = vec![name.to_owned()];
    let options = ApplyOptions {
        planes: &planes,
        actor: ChangeActor::Cli,
        reason: "deploy",
        dry_run: false,
        wait: RECONCILE_LOCK_WAIT,
    };

    let first = apply.apply(&options).await.expect("first apply");
    assert_eq!(first.applied(), 2);

    sqlx::query("UPDATE plane_e2e_rows SET value = 'console' WHERE plane = $1 AND entity = 'a'")
        .bind(name)
        .execute(&*h.pool.write_pool())
        .await
        .expect("console edit");
    let boot = h.service.reconcile_all(ReconcileTrigger::Boot).await;
    assert_eq!(boot.planes[0].outcome, PlaneOutcome::DriftReported);
    let again = apply.apply(&options).await.expect("second apply");
    assert_eq!(again.planes[0].drift, vec!["a"]);
    assert_eq!(
        stored_rows(&h.pool, name)
            .await
            .get("a")
            .map(String::as_str),
        Some("console")
    );
    assert_eq!(
        h.service.diff(name).await.expect("diff").effect,
        PlaneEffect::OnRestart
    );
}

#[tokio::test]
async fn an_apply_that_does_not_converge_rolls_back_rows_and_audit() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut plane = DbPlane::new(unique_plane_name("e2e_stuck"), &new_log());
    plane.corrupt = true;
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2")]);
    let name = plane.name;
    let h = harness(root.path(), vec![Arc::new(plane)], &[]).await;
    seed_rows(&h.pool, name, &[("a", "live")]).await;
    let change = ChangeRequest {
        actor: ChangeActor::Cli,
        reason: "overwrite",
    };

    h.service
        .apply(name, ApplyMode::Overwrite, &change, EntityScope::All)
        .await
        .expect_err("not converged");

    assert_eq!(stored_rows(&h.pool, name).await, snapshot(&[("a", "live")]));
    assert!(
        h.service
            .repository()
            .list_changes(name, 10)
            .await
            .expect("audit")
            .is_empty()
    );
}

#[tokio::test]
async fn a_scoped_apply_writes_and_audits_only_the_named_entities() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = DbPlane::new(unique_plane_name("e2e_scope"), &new_log());
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2")]);
    let name = plane.name;
    let h = harness(root.path(), vec![Arc::new(plane)], &[name]).await;
    seed_rows(&h.pool, name, &[("stray", "x")]).await;
    let change = ChangeRequest {
        actor: ChangeActor::Cli,
        reason: "only a",
    };
    let keys = vec!["a".to_owned()];

    let applied = h
        .service
        .apply(
            name,
            ApplyMode::Overwrite,
            &change,
            EntityScope::Entities(&keys),
        )
        .await
        .expect("scoped apply");

    assert_eq!((applied.changes, applied.remaining_drift), (1, 0));
    assert_eq!(
        stored_rows(&h.pool, name).await,
        snapshot(&[("a", "1"), ("stray", "x")])
    );
    let audit = h
        .service
        .repository()
        .list_changes(name, 10)
        .await
        .expect("audit");
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].entity, "a");
}

#[tokio::test]
async fn an_extension_plane_depending_on_an_unregistered_plane_fails_assembly() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut plane = DbPlane::new(unique_plane_name("e2e_orphan"), &new_log());
    plane.deps = &["no_such_plane"];
    let pool = test_db_pool().await;
    let mut config = fixture_config(&test_database_url());
    config.services_path = root.path().display().to_string();
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(PlanesExtension {
            id: unique_plane_name("planes_ext_orphan"),
            planes: vec![Arc::new(plane)],
        }))
        .expect("register extension");
    let ctx = fixture_app_context_with_extensions(&pool, config, registry).expect("context");

    let err = ExtensionContext::config_planes(&*ctx).expect_err("refused");
    assert!(matches!(
        err,
        ConfigPlaneError::UnknownDependency { ref dependency, .. } if dependency == "no_such_plane"
    ));
}

#[tokio::test]
async fn a_preview_apply_reports_the_changes_and_writes_nothing() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = DbPlane::new(unique_plane_name("e2e_preview"), &new_log());
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2")]);
    let name = plane.name;
    let h = harness(root.path(), vec![Arc::new(plane)], &[name]).await;
    seed_rows(&h.pool, name, &[("a", "live")]).await;
    let change = ChangeRequest {
        actor: ChangeActor::Cli,
        reason: "preview",
    };

    let preview = h
        .service
        .preview_apply(name, ApplyMode::Overwrite, &change, EntityScope::All)
        .await
        .expect("preview");

    assert_eq!((preview.changes, preview.remaining_drift), (2, 0));
    assert_eq!(stored_rows(&h.pool, name).await, snapshot(&[("a", "live")]));
    assert!(
        h.service
            .repository()
            .list_changes(name, 10)
            .await
            .expect("audit")
            .is_empty()
    );
}

fn options<'a>(planes: &'a [String], reason: &'a str) -> ApplyOptions<'a> {
    ApplyOptions {
        planes,
        actor: ChangeActor::Cli,
        reason,
        dry_run: false,
        wait: RECONCILE_LOCK_WAIT,
    }
}

#[tokio::test]
async fn a_file_authority_apply_overwrites_console_rows_and_audits_overwrite() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = DbPlane::new(unique_plane_name("e2e_fwins"), &new_log());
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2")]);
    let name = plane.name;
    let h = harness_with(
        root.path(),
        vec![Arc::new(plane)],
        &[(name, PlaneAuthority::File)],
    )
    .await;
    seed_rows(&h.pool, name, &[("a", "console"), ("extra", "x")]).await;
    let planes = vec![name.to_owned()];

    let report = ConfigApplyService::new(h.service.clone())
        .apply(&options(&planes, "gitops"))
        .await
        .expect("apply");

    let merged = &report.planes[0];
    assert!(merged.file_wins);
    assert!(!merged.adopted);
    assert_eq!(merged.applied, vec!["a", "b", "extra"]);
    assert!(merged.drift.is_empty() && merged.conflicts.is_empty());
    assert_eq!(
        stored_rows(&h.pool, name).await,
        snapshot(&[("a", "1"), ("b", "2")])
    );
    let audit = h
        .service
        .repository()
        .list_changes(name, 10)
        .await
        .expect("audit");
    assert_eq!(audit.len(), 3);
    assert!(audit.iter().all(|row| row.mode == "overwrite"));
}

#[tokio::test]
async fn switching_a_conflicted_plane_to_file_authority_resolves_its_conflicts_to_the_file() {
    let root = tempfile::tempdir().expect("tempdir");
    let name = unique_plane_name("e2e_fresolve");
    let first = DbPlane::new(name, &new_log());
    first.write_declaration(root.path(), &[("a", "1")]);
    let planes = vec![name.to_owned()];

    let db = harness(root.path(), vec![Arc::new(first)], &[name]).await;
    let db_apply = ConfigApplyService::new(db.service.clone());
    db_apply
        .apply(&options(&planes, "seed"))
        .await
        .expect("first apply");
    sqlx::query("UPDATE plane_e2e_rows SET value = 'console' WHERE plane = $1 AND entity = 'a'")
        .bind(name)
        .execute(&*db.pool.write_pool())
        .await
        .expect("console edit");
    DbPlane::new(name, &new_log()).write_declaration(root.path(), &[("a", "2")]);
    let conflicted = db_apply
        .apply(&options(&planes, "both moved"))
        .await
        .expect("conflicting apply");
    assert_eq!(conflicted.conflicts(), 1);

    let file = harness_with(
        root.path(),
        vec![Arc::new(DbPlane::new(name, &new_log()))],
        &[(name, PlaneAuthority::File)],
    )
    .await;
    let resolved = ConfigApplyService::new(file.service.clone())
        .apply_file_managed(ChangeActor::Reconciler, "gitops", RECONCILE_LOCK_WAIT)
        .await
        .expect("file wins");

    assert_eq!(resolved.conflicts(), 0);
    assert_eq!(resolved.planes[0].applied, vec!["a"]);
    assert_eq!(stored_rows(&file.pool, name).await, snapshot(&[("a", "2")]));
    assert!(
        file.service
            .repository()
            .list_open_conflicts(Some(name))
            .await
            .expect("conflicts")
            .is_empty()
    );
}

#[tokio::test]
async fn apply_file_managed_leaves_db_authority_planes_alone() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = DbPlane::new(unique_plane_name("e2e_fdbonly"), &new_log());
    plane.write_declaration(root.path(), &[("a", "1")]);
    let name = plane.name;
    let h = harness(root.path(), vec![Arc::new(plane)], &[]).await;
    seed_rows(&h.pool, name, &[("a", "console")]).await;

    let report = ConfigApplyService::new(h.service.clone())
        .apply_file_managed(ChangeActor::Boot, "boot", RECONCILE_LOCK_WAIT)
        .await
        .expect("no-op");

    assert!(report.planes.is_empty());
    assert_eq!(
        stored_rows(&h.pool, name).await,
        snapshot(&[("a", "console")])
    );
}
