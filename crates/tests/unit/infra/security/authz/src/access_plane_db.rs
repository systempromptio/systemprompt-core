//! The `access_control` config plane against the database: a declaration
//! written through the plane reads back identical, carries its origin, keeps
//! per-user rows, survives the gateway catalog prune when it is a glob, and
//! renders back as `rules.yaml`.

use std::path::Path;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::{
    ConfigPlane, ConfigPlaneError, PlaneContext, PlaneSnapshot, PlaneTx, ResolvedEntities,
    ResolvedEntity,
};
use systemprompt_governance::authz::types::{Access, EntityKind, RuleType};
use systemprompt_governance::authz::{
    AccessControlPlane, AccessControlRepository, UpsertRuleParams,
    reconcile_gateway_entities_exact, render_current_rules,
};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::Dependencies;

type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

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

    fn declare(&self, body: &str) {
        let path = self.root.path().join("access-control/rules.yaml");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, body.replace("TAG", &self.tag)).expect("write rules");
    }

    fn key(&self, template: &str) -> String {
        template.replace("TAG", &self.tag)
    }

    async fn write_declared(&self) -> PlaneSnapshot {
        let declared = AccessControlPlane
            .sources(self.root.path())
            .expect("sources")
            .expect("declared");
        let resolved: ResolvedEntities = declared
            .iter()
            .map(|(k, d)| {
                (
                    k.clone(),
                    ResolvedEntity {
                        value: Some(d.value.clone()),
                        source: d.source.clone(),
                    },
                )
            })
            .collect();
        self.write(&resolved).await;
        declared.into_iter().map(|(k, d)| (k, d.value)).collect()
    }

    async fn write(&self, resolved: &ResolvedEntities) {
        let deps = Dependencies::new();
        let ctx = PlaneContext::new(self.root.path(), &deps);
        let mut tx: Tx = self.pool.write_pool().begin().await.expect("begin");
        AccessControlPlane
            .write(&ctx, &mut PlaneTx::new(&mut tx), resolved)
            .await
            .expect("write");
        tx.commit().await.expect("commit");
    }

    async fn read(&self) -> PlaneSnapshot {
        let deps = Dependencies::new();
        let ctx = PlaneContext::new(self.root.path(), &deps);
        let mut tx: Tx = self.pool.write_pool().begin().await.expect("begin");
        let current = AccessControlPlane
            .read_current(&ctx, &mut PlaneTx::new(&mut tx))
            .await
            .expect("read");
        current
            .into_iter()
            .filter(|(k, _)| k.contains(&self.tag))
            .collect()
    }

    async fn source_of(&self, entity_type: &str, entity_id: &str) -> Vec<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT source FROM access_control_rules WHERE entity_type = $1 AND entity_id = $2 \
             ORDER BY rule_value",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(&*self.pool.pool())
        .await
        .expect("sources")
    }
}

const RULES: &str = r#"
entities:
  - entity: marketplace/mkt-TAG
    default: open
    why: Everyone gets the commons.
    allow:
      role: [user]
  - entity: model/opus-TAG-*
    why: Opus for engineering.
    allow:
      group: [eng]
    deny:
      role: [contractor]
  - entity: provider/prov-TAG
    owner: bundle:kit
    why: The kit's account.
    allow:
      role: [user]
"#;

#[tokio::test]
async fn a_declaration_written_through_the_plane_reads_back_identical() {
    let f = Fixture::new().await;
    f.declare(RULES);

    let declared = f.write_declared().await;

    assert_eq!(f.read().await, declared);
    assert_eq!(
        f.source_of("provider", &f.key("prov-TAG")).await,
        ["bundle:kit"]
    );
    assert_eq!(
        f.source_of("model", &f.key("opus-TAG-*")).await,
        [
            "code:access-control/rules.yaml",
            "code:access-control/rules.yaml"
        ]
    );
    let rendered = render_current_rules(&f.pool).await.expect("render");
    assert!(rendered.contains(&f.key("model/opus-TAG-*")), "{rendered}");
}

#[tokio::test]
async fn removing_a_declaration_keeps_per_user_rules_and_catalog_rows() {
    let f = Fixture::new().await;
    f.declare(RULES);
    f.write_declared().await;
    let repo = AccessControlRepository::new(&f.pool);
    let marketplace = f.key("mkt-TAG");
    repo.upsert_rule(UpsertRuleParams {
        entity_type: EntityKind::Marketplace,
        entity_id: &marketplace,
        rule_type: RuleType::USER,
        rule_value: "someone",
        access: Access::Allow,
        justification: None,
        source: "console:admin",
    })
    .await
    .expect("per-user rule");

    let removed: ResolvedEntities = [f.key("marketplace/mkt-TAG"), f.key("model/opus-TAG-*")]
        .into_iter()
        .map(|k| {
            (
                k,
                ResolvedEntity {
                    value: None,
                    source: systemprompt_extension::prelude::ChangeSource::Migration,
                },
            )
        })
        .collect();
    f.write(&removed).await;

    let left = f.read().await;
    assert_eq!(
        left.keys().collect::<Vec<_>>(),
        [&f.key("provider/prov-TAG")]
    );
    let entity = repo
        .get_entity(EntityKind::Marketplace, &marketplace)
        .await
        .expect("lookup")
        .expect("catalog row kept");
    assert!(
        !entity.default_included,
        "a removed declaration closes the entity"
    );
    assert_eq!(
        f.source_of("marketplace", &marketplace).await,
        ["console:admin"]
    );
    assert!(
        repo.get_entity(EntityKind::Model, &f.key("opus-TAG-*"))
            .await
            .expect("lookup")
            .is_none(),
        "an entitlement with no rules left is deleted"
    );
}

#[tokio::test]
async fn a_route_glob_survives_the_gateway_catalog_prune() {
    let f = Fixture::new().await;
    f.declare(
        "entities:\n  - entity: gateway_route/glob-TAG-*\n    default: open\n    why: Open \
         routes.\n    allow:\n      role: [user]\n",
    );
    f.write_declared().await;
    let repo = AccessControlRepository::new(&f.pool);
    let route = f.key("glob-TAG-one");

    reconcile_gateway_entities_exact(&repo, &[route.as_str()], "code:services")
        .await
        .expect("catalog reconcile");

    assert_eq!(f.read().await.len(), 1, "the glob and its rules survive");
}

#[tokio::test]
async fn a_tree_still_carrying_roles_yaml_is_refused_with_the_migrate_hint() {
    let f = Fixture::new().await;
    f.declare(RULES);
    let roles = f.root.path().join("access-control/roles.yaml");
    std::fs::write(&roles, "rules: []\n").expect("roles");

    let err = AccessControlPlane
        .load_declared(Path::new(f.root.path()))
        .expect_err("refused");

    assert!(matches!(err, ConfigPlaneError::Invalid { .. }));
    assert!(err.to_string().contains("config migrate"), "{err}");
}

#[tokio::test]
async fn an_absent_rules_file_is_undeclared() {
    let f = Fixture::new().await;
    assert!(
        AccessControlPlane
            .load_declared(f.root.path())
            .expect("load")
            .is_none()
    );
}
