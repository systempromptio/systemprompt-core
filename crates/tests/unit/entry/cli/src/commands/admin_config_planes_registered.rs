use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_cli::admin::config::planes::{
    ApplyArgs, DiffArgs, ExportArgs, ModeArg, PlanesCommands, run,
};
use systemprompt_cli::shared::CommandOutput;
use systemprompt_extension::prelude::{
    ApplyOutcome, ApplyRequest, ConfigPlane, ConfigPlaneResult, DeclaredState, PlaneContext,
    PlaneSnapshot, PlaneTx,
};
use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRegistry};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    fixture_app_context_with_extensions, fixture_config, test_database_url, test_db_pool,
};

struct MemoryPlane {
    name: &'static str,
    path: &'static str,
    store: Mutex<PlaneSnapshot>,
}

#[async_trait]
impl ConfigPlane for MemoryPlane {
    fn name(&self) -> &'static str {
        self.name
    }

    fn declared_path(&self) -> &'static str {
        self.path
    }

    fn supports_scoped_apply(&self) -> bool {
        true
    }

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        let Ok(text) = std::fs::read_to_string(services_root.join(self.path)) else {
            return Ok(None);
        };
        Ok(Some(DeclaredState::from_entries(
            text.lines()
                .filter_map(|line| line.split_once('='))
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
        )))
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        _tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        Ok(self.store.lock().expect("lock").clone())
    }

    async fn apply(
        &self,
        _ctx: &PlaneContext<'_>,
        _tx: &mut PlaneTx<'_>,
        request: &ApplyRequest<'_>,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        let mut store = self.store.lock().expect("lock");
        let mut outcome = ApplyOutcome::default();
        for (k, v) in &request.declared.entries {
            if !request.scope.admits(k) {
                continue;
            }
            if store.insert(k.clone(), v.clone()).is_none() {
                outcome.inserted += 1;
            } else {
                outcome.updated += 1;
            }
        }
        Ok(outcome)
    }

    async fn export(
        &self,
        _ctx: &PlaneContext<'_>,
        _tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<Option<String>> {
        let store = self.store.lock().expect("lock");
        if store.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            store.iter().map(|(k, v)| format!("{k}={v}\n")).collect(),
        ))
    }
}

struct PlanesExtension {
    id: &'static str,
    plane: Arc<MemoryPlane>,
}

impl Extension for PlanesExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "CLI planes",
            version: "1.0.0",
        }
    }

    fn config_planes(&self) -> Vec<Arc<dyn ConfigPlane>> {
        vec![Arc::clone(&self.plane) as Arc<dyn ConfigPlane>]
    }
}

fn unique(prefix: &str) -> &'static str {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    Box::leak(
        format!(
            "{prefix}_{}_{}_{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed),
            uuid::Uuid::new_v4().simple()
        )
        .into_boxed_str(),
    )
}

struct Harness {
    root: tempfile::TempDir,
    plane: Arc<MemoryPlane>,
    ctx: Arc<AppContext>,
}

async fn harness(declared: Option<&str>, stored: &[(&str, &str)]) -> Harness {
    let root = tempfile::tempdir().expect("tempdir");
    let name = unique("cli_plane");
    let plane = Arc::new(MemoryPlane {
        name,
        path: Box::leak(format!("planes/{name}.txt").into_boxed_str()),
        store: Mutex::new(
            stored
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        ),
    });
    if let Some(body) = declared {
        let path = root.path().join(plane.path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, body).expect("declaration");
    }
    let pool = test_db_pool().await;
    let mut config = fixture_config(&test_database_url());
    config.services_path = root.path().display().to_string();
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(PlanesExtension {
            id: unique("cli_planes_ext"),
            plane: Arc::clone(&plane),
        }))
        .expect("register");
    let ctx = fixture_app_context_with_extensions(&pool, config, registry).expect("context");
    Harness { root, plane, ctx }
}

fn rendered(output: &CommandOutput) -> String {
    serde_json::to_string(output.artifact()).expect("artifact serialises")
}

fn stored(h: &Harness) -> PlaneSnapshot {
    h.plane.store.lock().expect("lock").clone()
}

#[tokio::test]
async fn list_shows_a_registered_plane_with_its_declared_hash() {
    let h = harness(Some("a=1\n"), &[]).await;
    let out = run(PlanesCommands::List, &h.ctx).await.expect("list");
    let title = out.title().expect("title");
    assert!(
        title.starts_with("Config planes — NOT APPLIED: "),
        "{title}"
    );
    assert!(title.contains(h.plane.name), "{title}");
    assert!(
        title.contains("run `systemprompt admin config apply`"),
        "{title}"
    );
    let text = rendered(&out);
    assert!(text.contains(h.plane.name), "{text}");
    assert!(text.contains(h.plane.path), "{text}");
}

#[tokio::test]
async fn list_titles_plainly_once_every_plane_is_applied() {
    let h = harness(
        Some(
            "a=1
",
        ),
        &[],
    )
    .await;
    systemprompt_test_fixtures::record_plane_applied(h.ctx.db_pool(), h.plane.name)
        .await
        .expect("record the apply");
    let out = run(PlanesCommands::List, &h.ctx).await.expect("list");
    let title = out.title().expect("title");
    assert!(!title.contains("NOT APPLIED"), "{title}");
}

#[tokio::test]
async fn diff_counts_the_rows_that_differ_from_the_declaration() {
    let h = harness(Some("a=1\nb=2\n"), &[("a", "1"), ("c", "3")]).await;
    let out = run(
        PlanesCommands::Diff(DiffArgs {
            plane: h.plane.name.to_owned(),
        }),
        &h.ctx,
    )
    .await
    .expect("diff");
    let title = out.title().expect("titled").to_owned();
    assert!(title.starts_with(h.plane.name), "{title}");
    assert!(title.ends_with(": 2 drift rows"), "{title}");
}

#[tokio::test]
async fn apply_writes_the_declaration_and_a_scoped_apply_touches_only_its_entities() {
    let h = harness(Some("a=1\nb=2\n"), &[]).await;
    let scoped = run(
        PlanesCommands::Apply(ApplyArgs {
            plane: h.plane.name.to_owned(),
            reason: " cli scoped ".to_owned(),
            mode: ModeArg::Overwrite,
            entities: vec!["a".to_owned()],
        }),
        &h.ctx,
    )
    .await
    .expect("scoped apply");
    assert!(
        rendered(&scoped).contains("1 entities changed"),
        "{}",
        rendered(&scoped)
    );
    assert_eq!(stored(&h).get("a").map(String::as_str), Some("1"));
    assert!(!stored(&h).contains_key("b"), "b was outside the scope");

    run(
        PlanesCommands::Apply(ApplyArgs {
            plane: h.plane.name.to_owned(),
            reason: "cli full".to_owned(),
            mode: ModeArg::InsertOnly,
            entities: Vec::new(),
        }),
        &h.ctx,
    )
    .await
    .expect("full apply");
    assert_eq!(stored(&h).get("b").map(String::as_str), Some("2"));
}

#[tokio::test]
async fn seed_if_empty_leaves_a_populated_plane_alone() {
    let h = harness(Some("a=declared\n"), &[("a", "live")]).await;
    run(
        PlanesCommands::Apply(ApplyArgs {
            plane: h.plane.name.to_owned(),
            reason: "seed".to_owned(),
            mode: ModeArg::SeedIfEmpty,
            entities: Vec::new(),
        }),
        &h.ctx,
    )
    .await
    .expect("seed apply");
    assert_eq!(stored(&h).get("a").map(String::as_str), Some("live"));
}

#[tokio::test]
async fn export_prints_writes_and_refuses_to_clobber_without_force() {
    let h = harness(None, &[("k", "v")]).await;
    let export = |out: Option<std::path::PathBuf>, force: bool| {
        PlanesCommands::Export(ExportArgs {
            plane: h.plane.name.to_owned(),
            out,
            force,
        })
    };

    let printed = run(export(None, false), &h.ctx).await.expect("print");
    assert!(rendered(&printed).contains("k=v"), "{}", rendered(&printed));

    let target = h.root.path().join("nested/out/plane.txt");
    run(export(Some(target.clone()), false), &h.ctx)
        .await
        .expect("write");
    assert_eq!(std::fs::read_to_string(&target).expect("read"), "k=v\n");

    let err = run(export(Some(target.clone()), false), &h.ctx)
        .await
        .expect_err("existing file without --force");
    assert!(err.to_string().contains("--force"), "{err}");

    std::fs::write(&target, "stale").expect("overwrite fixture");
    run(export(Some(target.clone()), true), &h.ctx)
        .await
        .expect("forced write");
    assert_eq!(std::fs::read_to_string(&target).expect("read"), "k=v\n");
}

#[tokio::test]
async fn export_of_an_empty_plane_writes_nothing() {
    let h = harness(None, &[]).await;
    let target = h.root.path().join("never.txt");
    let out = run(
        PlanesCommands::Export(ExportArgs {
            plane: h.plane.name.to_owned(),
            out: Some(target.clone()),
            force: false,
        }),
        &h.ctx,
    )
    .await
    .expect("empty export");
    assert!(
        rendered(&out).contains("nothing written"),
        "{}",
        rendered(&out)
    );
    assert!(!target.exists());
}
