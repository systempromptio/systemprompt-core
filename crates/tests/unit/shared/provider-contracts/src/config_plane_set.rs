use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_provider_contracts::{
    ApplyOutcome, ApplyRequest, ConfigPlane, ConfigPlaneError, ConfigPlaneResult, ConfigPlaneSet,
    DeclaredState, EffectiveAuthority, PlaneAuthority, PlaneContext, PlaneSnapshot, PlaneTx,
};

struct StubPlane {
    name: &'static str,
    path: &'static str,
    deps: &'static [&'static str],
}

#[async_trait]
impl ConfigPlane for StubPlane {
    fn name(&self) -> &'static str {
        self.name
    }

    fn declared_path(&self) -> &'static str {
        self.path
    }

    fn depends_on(&self) -> &'static [&'static str] {
        self.deps
    }

    fn load_declared(&self, _root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        Ok(None)
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        _tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        Ok(PlaneSnapshot::new())
    }

    async fn apply(
        &self,
        _ctx: &PlaneContext<'_>,
        _tx: &mut PlaneTx<'_>,
        _request: &ApplyRequest<'_>,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        Ok(ApplyOutcome::default())
    }
}

fn plane(name: &'static str, deps: &'static [&'static str]) -> Arc<dyn ConfigPlane> {
    Arc::new(StubPlane {
        name,
        path: "planes/stub.yaml",
        deps,
    })
}

fn set_of(planes: Vec<Arc<dyn ConfigPlane>>) -> ConfigPlaneResult<ConfigPlaneSet> {
    ConfigPlaneSet::new(planes, BTreeMap::new(), "/nonexistent".into())
}

#[test]
fn planes_are_ordered_after_their_dependencies_keeping_registration_order() {
    let set = set_of(vec![
        plane("access_control", &["groups"]),
        plane("routes", &[]),
        plane("groups", &[]),
        plane("policies", &["access_control", "groups"]),
    ])
    .expect("set");
    assert_eq!(
        set.names(),
        vec!["routes", "groups", "access_control", "policies"]
    );
}

#[test]
fn a_dependency_on_an_unregistered_plane_is_refused() {
    let err = set_of(vec![plane("access_control", &["groups"])]).expect_err("refused");
    assert!(matches!(
        err,
        ConfigPlaneError::UnknownDependency { ref plane, ref dependency }
            if plane == "access_control" && dependency == "groups"
    ));
}

#[test]
fn a_dependency_cycle_is_refused() {
    let err = set_of(vec![
        plane("a", &["b"]),
        plane("b", &["a"]),
        plane("c", &[]),
    ])
    .expect_err("refused");
    assert!(matches!(err, ConfigPlaneError::DependencyCycle(ref p) if p == "a"));
    let err = set_of(vec![plane("self_ref", &["self_ref"])]).expect_err("refused");
    assert!(matches!(err, ConfigPlaneError::DependencyCycle(ref p) if p == "self_ref"));
}

#[test]
fn duplicate_names_are_refused_before_ordering() {
    let err = set_of(vec![plane("dup", &[]), plane("dup", &[])]).expect_err("refused");
    assert!(matches!(err, ConfigPlaneError::DuplicatePlane(ref p) if p == "dup"));
}

#[test]
fn the_writer_is_refused_for_a_file_managed_plane_and_minted_otherwise() {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(root.path().join("planes")).expect("mkdir");
    std::fs::write(root.path().join("planes/stub.yaml"), "a: 1\n").expect("write");
    let mut authority = BTreeMap::new();
    authority.insert("db_plane".to_owned(), PlaneAuthority::Db);
    let set = ConfigPlaneSet::new(
        vec![plane("file_plane", &[]), plane("db_plane", &[])],
        authority,
        root.path().to_path_buf(),
    )
    .expect("set");

    let err = set.writer("file_plane").expect_err("file-managed");
    assert!(matches!(
        err,
        ConfigPlaneError::FileManaged { ref plane, ref path }
            if plane == "file_plane" && path == "planes/stub.yaml"
    ));
    let writer = set.writer("db_plane").expect("db authority is writable");
    assert_eq!(writer.name(), "db_plane");
    assert_eq!(writer.authority(), EffectiveAuthority::Db);
    assert!(matches!(
        set.writer("missing"),
        Err(ConfigPlaneError::UnknownPlane(ref p)) if p == "missing"
    ));
}

#[test]
fn a_file_authority_plane_without_its_file_is_writable_as_undeclared() {
    let set = set_of(vec![plane("absent", &[])]).expect("set");
    let writer = set.writer("absent").expect("writable");
    assert_eq!(writer.authority(), EffectiveAuthority::Undeclared);
}

#[tokio::test]
async fn export_and_scoped_apply_are_refused_by_default() {
    let stub = plane("plain", &[]);
    assert!(!stub.supports_scoped_apply());
    let deps = systemprompt_provider_contracts::Dependencies::new();
    let ctx = PlaneContext::new(Path::new("/nonexistent"), &deps);
    let mut handle = 0_u8;
    let mut tx = PlaneTx::new(&mut handle);
    let err = stub.export(&ctx, &mut tx).await.expect_err("refused");
    assert!(matches!(
        err,
        ConfigPlaneError::Unsupported { ref plane, operation: "export" } if plane == "plain"
    ));
}

#[test]
fn a_plane_tx_hands_back_only_the_type_the_host_inserted() {
    let mut handle = String::from("tx");
    let mut tx = PlaneTx::new(&mut handle);
    assert!(tx.get::<u64>().is_err());
    tx.get::<String>().expect("inserted type").push('!');
    assert_eq!(handle, "tx!");
}

#[test]
fn trait_defaults_are_live_unscoped_and_diff_by_snapshot() {
    let plane = StubPlane {
        name: "defaults",
        path: "defaults.yaml",
        deps: &[],
    };
    assert_eq!(
        plane.effect(),
        systemprompt_provider_contracts::PlaneEffect::Live
    );
    assert!(!plane.supports_scoped_apply());

    let mut declared = PlaneSnapshot::new();
    declared.insert("a".to_owned(), "1".to_owned());
    declared.insert("b".to_owned(), "2".to_owned());
    let mut current = PlaneSnapshot::new();
    current.insert("a".to_owned(), "1".to_owned());
    current.insert("c".to_owned(), "3".to_owned());
    let state = DeclaredState::from_entries(declared.clone());

    let rows = plane.diff(&state, &current);
    assert_eq!(
        rows,
        systemprompt_provider_contracts::diff_snapshots(&declared, &current)
    );
    assert_eq!(rows.len(), 2, "b missing and c extra: {rows:?}");
    assert!(plane.diff(&state, &declared).is_empty());
}

#[test]
fn plane_context_exposes_the_root_and_its_dependencies() {
    let deps = systemprompt_provider_contracts::Dependencies::new().with(7_u32);
    let root = Path::new("/srv/services");
    let ctx = PlaneContext::new(root, &deps);
    assert_eq!(ctx.services_root(), root);
    assert_eq!(*ctx.get::<u32>().expect("u32 present"), 7);
    assert!(ctx.get::<String>().is_err());
}
