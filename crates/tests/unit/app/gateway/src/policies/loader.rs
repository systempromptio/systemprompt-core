//! The `gateway_policies` config plane against the database: a declaration
//! written through the plane reads back with no drift, carries its origin,
//! renders back to the same entries, and an invalid file is refused.

use std::collections::BTreeMap;

use systemprompt_database::{DbPool, PlaneTransaction};
use systemprompt_extension::prelude::{
    ChangeSource, ConfigPlane, ConfigPlaneError, PlaneContext, PlaneSnapshot, PlaneTx,
    ResolvedEntities, ResolvedEntity,
};
use systemprompt_gateway::{GATEWAY_POLICIES_FILE, GatewayPoliciesPlane};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::Dependencies;

struct Fixture {
    pool: DbPool,
    root: tempfile::TempDir,
    tag: String,
}

impl Fixture {
    async fn new() -> Self {
        Self {
            pool: test_db_pool().await,
            root: tempfile::tempdir().expect("tempdir"),
            tag: uuid::Uuid::new_v4().simple().to_string()[..10].to_owned(),
        }
    }

    fn name(&self, base: &str) -> String {
        format!("{base}-{}", self.tag)
    }

    fn declare(&self, body: &str) {
        let path = self.root.path().join(GATEWAY_POLICIES_FILE);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, body.replace("TAG", &self.tag)).expect("write");
    }

    fn declared(&self) -> PlaneSnapshot {
        GatewayPoliciesPlane
            .load_declared(self.root.path())
            .expect("load")
            .expect("declared")
            .entries
    }

    async fn write(&self, resolved: &ResolvedEntities) {
        let deps = Dependencies::new();
        let ctx = PlaneContext::new(self.root.path(), &deps);
        let mut tx: PlaneTransaction = self.pool.write_pool().begin().await.expect("begin");
        GatewayPoliciesPlane
            .write(&ctx, &mut PlaneTx::new(&mut tx), resolved)
            .await
            .expect("write");
        tx.commit().await.expect("commit");
    }

    async fn read(&self) -> PlaneSnapshot {
        let deps = Dependencies::new();
        let ctx = PlaneContext::new(self.root.path(), &deps);
        let mut tx: PlaneTransaction = self.pool.write_pool().begin().await.expect("begin");
        GatewayPoliciesPlane
            .read_current(&ctx, &mut PlaneTx::new(&mut tx))
            .await
            .expect("read")
            .into_iter()
            .filter(|(k, _)| k.contains(&self.tag))
            .collect()
    }
}

const POLICIES: &str = r#"
policies:
  - name: quota-TAG
    priority: 5
    spec:
      safety:
        scanners: [custom_scanner]
        scanner_settings:
          custom_scanner:
            timeout_ms: 250
            config:
              zeta: 1
              alpha: { nested_b: true, nested_a: [3, 1] }
  - name: off-TAG
    enabled: false
"#;

fn resolved(entries: &PlaneSnapshot, source: &ChangeSource) -> ResolvedEntities {
    entries
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                ResolvedEntity {
                    value: Some(v.clone()),
                    source: source.clone(),
                },
            )
        })
        .collect()
}

#[tokio::test]
async fn a_written_declaration_reads_back_without_drift_despite_jsonb_key_order() {
    let f = Fixture::new().await;
    f.declare(POLICIES);
    let declared = f.declared();

    f.write(&resolved(
        &declared,
        &ChangeSource::Code(GATEWAY_POLICIES_FILE.to_owned()),
    ))
    .await;

    assert_eq!(f.read().await, declared);
    let source: String =
        sqlx::query_scalar("SELECT source FROM ai_gateway_policies WHERE name = $1")
            .bind(f.name("quota"))
            .fetch_one(&*f.pool.pool())
            .await
            .expect("source");
    assert_eq!(source, "code:gateway/policies.yaml");
}

#[tokio::test]
async fn rendering_reparses_to_the_same_entries() {
    let f = Fixture::new().await;
    f.declare(POLICIES);
    let declared = f.declared();

    let text = GatewayPoliciesPlane
        .render_declarations(&declared)
        .expect("render");
    f.declare(&text);

    assert_eq!(f.declared(), declared);
}

#[tokio::test]
async fn removing_a_policy_deletes_its_row() {
    let f = Fixture::new().await;
    f.declare(POLICIES);
    let declared = f.declared();
    f.write(&resolved(&declared, &ChangeSource::Migration))
        .await;

    let gone: ResolvedEntities = BTreeMap::from([(
        f.name("off"),
        ResolvedEntity {
            value: None,
            source: ChangeSource::Migration,
        },
    )]);
    f.write(&gone).await;

    let left = f.read().await;
    assert_eq!(left.keys().collect::<Vec<_>>(), [&f.name("quota")]);
}

#[tokio::test]
async fn an_invalid_declaration_is_refused() {
    let f = Fixture::new().await;
    f.declare("policies:\n  - name: dup-TAG\n  - name: dup-TAG\n");
    let err = GatewayPoliciesPlane
        .load_declared(f.root.path())
        .expect_err("duplicate");
    assert!(matches!(err, ConfigPlaneError::Invalid { .. }), "{err}");

    f.declare("policies:\n  - name: x\n    bogus: 1\n");
    assert!(matches!(
        GatewayPoliciesPlane.load_declared(f.root.path()),
        Err(ConfigPlaneError::Invalid { .. })
    ));
}

#[tokio::test]
async fn an_absent_file_is_undeclared() {
    let f = Fixture::new().await;
    assert!(
        GatewayPoliciesPlane
            .load_declared(f.root.path())
            .expect("load")
            .is_none()
    );
}
