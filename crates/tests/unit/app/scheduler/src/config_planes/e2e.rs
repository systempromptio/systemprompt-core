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
    ConfigReconcileService, PlaneOutcome, RECONCILE_LOCK_WAIT, ReconcileTrigger,
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
    let pool = test_db_pool().await;
    ensure_table(&pool).await;
    let mut config = fixture_config(&test_database_url());
    config.services_path = root.display().to_string();
    for name in db {
        config
            .planes
            .authority
            .insert((*name).to_owned(), PlaneAuthority::Db);
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
async fn boot_reconciles_registered_planes_in_dependency_order() {
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
    assert_eq!(*log.lock().expect("lock"), vec![parent_name, child_name]);
    assert_eq!(
        stored_rows(&h.pool, parent_name).await,
        snapshot(&[("g", "1")])
    );
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
        (1, "boot", "overwrite")
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
    let h = harness(
        root.path(),
        vec![Arc::new(file_plane), Arc::new(db_plane)],
        &[db_name],
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
async fn a_db_plane_is_seeded_once_and_console_edits_survive_later_passes() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut plane = DbPlane::new(unique_plane_name("e2e_seed"), &new_log());
    plane.effect = PlaneEffect::OnRestart;
    plane.write_declaration(root.path(), &[("a", "1"), ("b", "2")]);
    let name = plane.name;
    let h = harness(root.path(), vec![Arc::new(plane)], &[name]).await;

    let first = h.service.reconcile_all(ReconcileTrigger::Boot).await;
    assert_eq!(first.planes[0].outcome, PlaneOutcome::Seeded);
    let audit = h
        .service
        .repository()
        .list_changes(name, 10)
        .await
        .expect("audit");
    assert_eq!(audit.len(), 2);
    assert!(
        audit
            .iter()
            .all(|r| r.actor == "seed" && r.mode == "seed_if_empty")
    );

    sqlx::query("UPDATE plane_e2e_rows SET value = 'console' WHERE plane = $1 AND entity = 'a'")
        .bind(name)
        .execute(&*h.pool.write_pool())
        .await
        .expect("console edit");
    let second = h.service.reconcile_all(ReconcileTrigger::Boot).await;
    assert_eq!(second.planes[0].outcome, PlaneOutcome::DriftReported);
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

    let report = h.service.reconcile_all(ReconcileTrigger::Scheduled).await;

    assert_eq!(report.failures().len(), 1);
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
