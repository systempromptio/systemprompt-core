//! The explicit apply's three-way merge under two concurrent appliers
//! (credential.md §3.1 table, §9.4): for each of the four rows, two appliers
//! race on real database rows and exactly one of them records the outcome.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use systemprompt_database::DbPool;
use systemprompt_provider_contracts::{ChangeActor, ConfigPlane, Dependencies, PlaneSnapshot};
use systemprompt_runtime::ConfigPlaneSet;
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::{
    ApplyOptions, ConfigApplyService, ConfigReconcileService, MergeReport, PlaneMerge,
};
use systemprompt_test_fixtures::test_db_pool;

use super::db_plane::{DbPlane, ensure_table, seed_rows, stored_rows};
use super::fake_plane::unique_plane_name;

struct Race {
    pool: DbPool,
    name: &'static str,
    plane: Arc<DbPlane>,
    root: tempfile::TempDir,
}

impl Race {
    async fn new(prefix: &str) -> Self {
        let pool = test_db_pool().await;
        ensure_table(&pool).await;
        let plane = Arc::new(DbPlane::new(
            unique_plane_name(prefix),
            &Arc::new(Mutex::new(Vec::new())),
        ));
        Self {
            pool,
            name: plane.name,
            plane,
            root: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn declare(&self, lines: &[(&str, &str)]) {
        self.plane.write_declaration(self.root.path(), lines);
    }

    fn service(&self) -> ConfigApplyService {
        let planes: Vec<Arc<dyn ConfigPlane>> = vec![Arc::clone(&self.plane) as _];
        let set = ConfigPlaneSet::new(planes, Default::default(), self.root.path().to_path_buf())
            .expect("plane set");
        ConfigApplyService::new(ConfigReconcileService::new(
            set,
            ConfigReconcileRepository::new(&self.pool),
            Dependencies::new(),
        ))
    }

    async fn apply(&self, dry_run: bool) -> PlaneMerge {
        let report = self
            .service()
            .apply(&options(dry_run))
            .await
            .expect("apply");
        only(report)
    }

    async fn race(&self) -> (PlaneMerge, PlaneMerge) {
        let (first, second) = (self.service(), self.service());
        let opts = options(false);
        let (a, b) = tokio::join!(first.apply(&opts), second.apply(&opts));
        (
            only(a.expect("first applier")),
            only(b.expect("second applier")),
        )
    }

    async fn console_edit(&self, entity: &str, value: &str) {
        sqlx::query("UPDATE plane_e2e_rows SET value = $3 WHERE plane = $1 AND entity = $2")
            .bind(self.name)
            .bind(entity)
            .bind(value)
            .execute(&*self.pool.write_pool())
            .await
            .expect("console edit");
    }

    async fn audit_rows(&self) -> usize {
        self.service()
            .reconcile()
            .repository()
            .list_changes(self.name, 100)
            .await
            .expect("audit")
            .len()
    }

    async fn open_conflicts(&self) -> usize {
        self.service()
            .reconcile()
            .repository()
            .list_open_conflicts(Some(self.name))
            .await
            .expect("conflicts")
            .len()
    }
}

fn options(dry_run: bool) -> ApplyOptions<'static> {
    ApplyOptions {
        planes: &[],
        actor: ChangeActor::Cli,
        reason: "deploy",
        dry_run,
        wait: Duration::from_secs(30),
    }
}

fn only(report: MergeReport) -> PlaneMerge {
    assert_eq!(report.planes.len(), 1, "{report:?}");
    report.planes.into_iter().next().expect("one plane")
}

fn snapshot(pairs: &[(&str, &str)]) -> PlaneSnapshot {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn neither_side_moved_both_appliers_are_no_ops() {
    let r = Race::new("race_noop").await;
    r.declare(&[("a", "1")]);
    r.apply(false).await;
    let audit = r.audit_rows().await;

    let (x, y) = r.race().await;

    assert!(x.applied.is_empty() && y.applied.is_empty());
    assert_eq!((x.unchanged, y.unchanged), (1, 1));
    assert_eq!(r.audit_rows().await, audit);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_the_file_moved_exactly_one_applier_writes_it() {
    let r = Race::new("race_file").await;
    r.declare(&[("a", "1"), ("gone", "x")]);
    r.apply(false).await;
    let audit = r.audit_rows().await;
    r.declare(&[("a", "2"), ("b", "3")]);

    let (x, y) = r.race().await;

    let writers = [&x, &y].iter().filter(|m| !m.applied.is_empty()).count();
    assert_eq!(writers, 1, "{x:?} {y:?}");
    let winner = if x.applied.is_empty() { &y } else { &x };
    assert_eq!(winner.applied, vec!["a", "b", "gone"]);
    assert_eq!(r.audit_rows().await, audit + 3);
    assert_eq!(
        stored_rows(&r.pool, r.name).await,
        snapshot(&[("a", "2"), ("b", "3")])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_the_database_moved_the_console_edit_is_kept_as_drift() {
    let r = Race::new("race_db").await;
    r.declare(&[("a", "1")]);
    r.apply(false).await;
    r.console_edit("a", "console").await;
    let audit = r.audit_rows().await;

    let (x, y) = r.race().await;

    assert_eq!(
        (x.drift.clone(), y.drift.clone()),
        (vec!["a".to_owned()], vec!["a".to_owned()])
    );
    assert!(x.applied.is_empty() && y.applied.is_empty());
    assert_eq!(r.audit_rows().await, audit);
    assert_eq!(
        stored_rows(&r.pool, r.name).await,
        snapshot(&[("a", "console")])
    );

    let patches = r.service().export_patches(&[]).await.expect("export");
    assert_eq!(patches.len(), 1);
    assert_eq!(patches[0].text, "a=console\n");
    assert_eq!(patches[0].entities, vec!["a"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn both_moved_one_conflict_is_recorded_and_nothing_is_written() {
    let r = Race::new("race_both").await;
    r.declare(&[("a", "1")]);
    r.apply(false).await;
    r.console_edit("a", "console").await;
    r.declare(&[("a", "2")]);
    let audit = r.audit_rows().await;

    let (x, y) = r.race().await;

    assert_eq!((x.conflicts.len(), y.conflicts.len()), (1, 1));
    assert_eq!(r.open_conflicts().await, 1);
    assert_eq!(r.audit_rows().await, audit);
    assert_eq!(
        stored_rows(&r.pool, r.name).await,
        snapshot(&[("a", "console")])
    );

    r.console_edit("a", "2").await;
    let settled = r.apply(false).await;
    assert_eq!(settled.converged, vec!["a"]);
    assert_eq!(
        r.open_conflicts().await,
        0,
        "agreeing sides close the conflict"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_first_apply_adopts_the_database_and_only_inserts_what_it_lacks() {
    let r = Race::new("race_adopt").await;
    seed_rows(&r.pool, r.name, &[("a", "console"), ("mine", "m")]).await;
    r.declare(&[("a", "1"), ("b", "2")]);

    let first = r.apply(false).await;

    assert!(first.adopted);
    assert_eq!(first.applied, vec!["b"]);
    assert_eq!(first.drift, vec!["a", "mine"]);
    assert_eq!(
        stored_rows(&r.pool, r.name).await,
        snapshot(&[("a", "console"), ("b", "2"), ("mine", "m")])
    );
    let second = r.apply(false).await;
    assert!(!second.adopted);
    assert!(second.applied.is_empty());
    assert_eq!(second.drift, vec!["a", "mine"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_dry_run_reports_the_merge_and_writes_nothing() {
    let r = Race::new("race_dry").await;
    r.declare(&[("a", "1")]);

    let dry = r.apply(true).await;

    assert_eq!(dry.applied, vec!["a"]);
    assert!(stored_rows(&r.pool, r.name).await.is_empty());
    assert_eq!(r.audit_rows().await, 0);
    assert!(r.apply(false).await.adopted, "a dry run records no base");
}
