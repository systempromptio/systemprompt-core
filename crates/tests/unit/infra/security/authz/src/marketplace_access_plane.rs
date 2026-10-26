//! A marketplace's `access:` block is a declaration of the `access_control`
//! plane: `config apply` reads it as the `marketplace/<id>` entity with the
//! marketplace's own config file as its origin, and a boot or a reconcile
//! writes nothing for it.

use std::path::Path;

use systemprompt_extension::prelude::{ChangeSource, ConfigPlane};
use systemprompt_governance::authz::{AccessControlPlane, reconcile_services_authz};
use systemprompt_identifiers::MarketplaceId;
use systemprompt_manifest::services::{MarketplaceConfig, ServicesConfig};
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

fn marketplace_yaml(id: &str, access: &str) -> String {
    format!(
        "marketplace:\n  id: {id}\n  name: Test Market\n  description: d\n  version: 1.0.0\n  \
         license: MIT\n  author:\n    name: t\n    email: t@example.com\n{access}"
    )
}

fn write_marketplace(root: &Path, id: &str, access: &str) {
    let dir = root.join("marketplaces").join(id);
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("config.yaml"), marketplace_yaml(id, access)).expect("write");
}

#[test]
fn apply_declares_a_marketplace_access_block_with_its_config_file_as_origin() {
    let root = tempfile::tempdir().expect("tempdir");
    write_marketplace(
        root.path(),
        "ops",
        "  access:\n    default_included: true\n    roles: [admin]\n    justification: ops owns it\n    \
         rules:\n      - rule_type: adfs_group\n        values: [devs]\n        access: deny\n        \
         justification: devs are out\n",
    );

    let declared = AccessControlPlane
        .sources(root.path())
        .expect("sources")
        .expect("a marketplace block declares the plane");
    let entity = declared.get("marketplace/ops").expect("marketplace/ops");

    assert_eq!(
        entity.source,
        ChangeSource::Code("marketplaces/ops/config.yaml".to_owned())
    );
    let canonical: serde_json::Value = serde_json::from_str(&entity.value).expect("json");
    assert_eq!(canonical["default_included"], true);
    let rules = canonical["rules"].as_array().expect("rules");
    assert!(rules.iter().any(|r| r["rule_type"] == "role"
        && r["rule_value"] == "admin"
        && r["access"] == "allow"
        && r["why"] == "ops owns it"));
    assert!(rules.iter().any(|r| r["rule_type"] == "adfs_group"
        && r["rule_value"] == "devs"
        && r["access"] == "deny"
        && r["why"] == "devs are out"));
}

#[test]
fn a_marketplace_without_an_access_block_declares_nothing() {
    let root = tempfile::tempdir().expect("tempdir");
    write_marketplace(root.path(), "plain", "");

    assert!(
        AccessControlPlane
            .sources(root.path())
            .expect("sources")
            .is_none(),
        "no rules.yaml and no declared access leaves the plane undeclared"
    );
}

#[test]
fn an_entity_declared_in_both_places_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    write_marketplace(root.path(), "ops", "  access:\n    roles: [admin]\n");
    let rules = root.path().join("access-control/rules.yaml");
    std::fs::create_dir_all(rules.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        rules,
        "entities:\n  - entity: marketplace/ops\n    why: twice\n    allow:\n      role: [admin]\n",
    )
    .expect("write rules");

    let error = AccessControlPlane
        .sources(root.path())
        .expect_err("two declarations of one entity");
    assert!(error.to_string().contains("marketplace/ops"), "{error}");
}

#[test]
fn a_reserved_rule_type_in_the_block_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    write_marketplace(
        root.path(),
        "ops",
        "  access:\n    rules:\n      - rule_type: user\n        values: [someone]\n",
    );

    assert!(AccessControlPlane.sources(root.path()).is_err());
}

#[tokio::test]
async fn a_reconcile_writes_no_marketplace_access() {
    let db = test_db_pool().await;
    let id = format!("boot-mkt-{}", Uuid::new_v4().simple());
    let cfg: MarketplaceConfig = serde_yaml::from_str(&format!(
        "id: {id}\nname: Test Market\ndescription: d\nversion: 1.0.0\nlicense: MIT\nauthor:\n  \
         name: t\n  email: t@example.com\naccess:\n  roles: [ops]\n"
    ))
    .expect("marketplace yaml");
    let mut services = ServicesConfig::default();
    services.marketplaces.insert(MarketplaceId::new(&id), cfg);

    let report = reconcile_services_authz(&db, &services, "bundle:test")
        .await
        .expect("reconcile");
    assert!(report.gateway.is_none());

    let pg = db.write_pool();
    let rows: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM access_control_rules WHERE entity_id = $1) + (SELECT \
         COUNT(*) FROM access_control_entities WHERE entity_id = $1)",
    )
    .bind(&id)
    .fetch_one(&*pg)
    .await
    .expect("count");
    assert_eq!(
        rows, 0,
        "boot applies nothing: only `config apply` writes access"
    );
}
