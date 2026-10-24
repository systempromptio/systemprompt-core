//! Two bundles composed into one root: each owns its own marketplaces, and
//! neither prune reaches the other's rows.
//!
//! A bundle that owns nothing is the dangerous edge. Its scope must still be a
//! scope: an ownership set that collapses to "unscoped" would turn the bundle
//! that claims least into the one that prunes most.

use std::collections::HashMap;

use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_governance::authz::{EntityKind, IngestScope, reconcile_composed_bundles};
use systemprompt_identifiers::MarketplaceId;
use systemprompt_manifest::services::{
    BundleOwnership, BundleSourceInfo, MarketplaceConfig, ServicesBundleManifest, ServicesConfig,
};
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

fn manifest(marketplaces: Vec<String>) -> ServicesBundleManifest {
    ServicesBundleManifest {
        format: 1,
        version: "1.0.0".to_owned(),
        created_at: Utc::now(),
        requires_core: ">=0.49".to_owned(),
        source: BundleSourceInfo::default(),
        files: Vec::new(),
        content_hash: String::new(),
        total_size: 0,
        owns: BundleOwnership {
            marketplaces,
            ..BundleOwnership::default()
        },
    }
}

fn marketplace(id: &str, role: &str) -> (MarketplaceId, MarketplaceConfig) {
    let cfg: MarketplaceConfig = serde_yaml::from_str(&format!(
        "id: {id}\nname: Test Market\ndescription: d\nversion: 1.0.0\nlicense: MIT\nauthor:\n  \
         name: t\n  email: t@example.com\naccess:\n  roles: [{role}]\n"
    ))
    .expect("marketplace yaml");
    (MarketplaceId::new(id), cfg)
}

async fn rules_for(db: &DbPool, entity_id: &str) -> Vec<(String, String)> {
    let pg = db.write_pool();
    sqlx::query_as::<_, (String, String)>(
        "SELECT rule_value, source FROM access_control_rules WHERE entity_id = $1 ORDER BY \
         rule_value",
    )
    .bind(entity_id)
    .fetch_all(&*pg)
    .await
    .expect("read back rules")
}

async fn cleanup(db: &DbPool, entity_id: &str) {
    let pg = db.write_pool();
    sqlx::query("DELETE FROM access_control_rules WHERE entity_id = $1")
        .bind(entity_id)
        .execute(&*pg)
        .await
        .expect("cleanup rules");
    sqlx::query("DELETE FROM access_control_entities WHERE entity_id = $1")
        .bind(entity_id)
        .execute(&*pg)
        .await
        .expect("cleanup entities");
}

#[tokio::test]
async fn each_bundle_owns_only_its_own_marketplaces() {
    let db = test_db_pool().await;
    let base_market = unique_id("bun-base");
    let extra_market = unique_id("bun-extra");

    let mut services = ServicesConfig::default();
    let mut markets = HashMap::new();
    let (id, cfg) = marketplace(&base_market, "ops");
    markets.insert(id, cfg);
    let (id, cfg) = marketplace(&extra_market, "analysts");
    markets.insert(id, cfg);
    services.marketplaces = markets;

    let base = manifest(vec![base_market.clone()]);
    let extra = manifest(vec![extra_market.clone()]);
    let bundles = [("base", &base), ("extra", &extra)];

    let reports = reconcile_composed_bundles(&db, &services, &bundles)
        .await
        .expect("first reconcile");

    assert_eq!(
        reports
            .iter()
            .map(|(name, report)| (
                name.as_str(),
                report
                    .marketplaces
                    .as_ref()
                    .expect("marketplace pass ran")
                    .inserted
            ))
            .collect::<Vec<_>>(),
        vec![("base", 1), ("extra", 1)],
        "each bundle projects exactly the marketplace it owns"
    );
    assert_eq!(
        rules_for(&db, &base_market).await,
        vec![("ops".to_owned(), "bundle:base".to_owned())],
        "the base marketplace grant is stamped with the base bundle"
    );
    assert_eq!(
        rules_for(&db, &extra_market).await,
        vec![("analysts".to_owned(), "bundle:extra".to_owned())],
        "the extra marketplace grant is stamped with the extra bundle"
    );

    cleanup(&db, &base_market).await;
    cleanup(&db, &extra_market).await;
}

#[tokio::test]
async fn one_bundles_prune_never_reaches_another_bundles_rows() {
    let db = test_db_pool().await;
    let mine = unique_id("bun-mine");
    let theirs = unique_id("bun-theirs");

    let services_with = |mine_role: &str, theirs_role: &str| -> ServicesConfig {
        let mut services = ServicesConfig::default();
        let mut markets = HashMap::new();
        let (id, cfg) = marketplace(&mine, mine_role);
        markets.insert(id, cfg);
        let (id, cfg) = marketplace(&theirs, theirs_role);
        markets.insert(id, cfg);
        services.marketplaces = markets;
        services
    };

    let base = manifest(vec![mine.clone()]);
    let extra = manifest(vec![theirs.clone()]);
    let bundles = [("base", &base), ("extra", &extra)];

    reconcile_composed_bundles(&db, &services_with("stale", "keepme"), &bundles)
        .await
        .expect("seed both bundles");

    let only_base = [("base", &base)];
    reconcile_composed_bundles(&db, &services_with("fresh", "keepme"), &only_base)
        .await
        .expect("re-reconcile the base bundle alone");

    assert_eq!(
        rules_for(&db, &mine).await,
        vec![("fresh".to_owned(), "bundle:base".to_owned())],
        "the base bundle's own prune drops the grant it stopped declaring"
    );
    assert_eq!(
        rules_for(&db, &theirs).await,
        vec![("keepme".to_owned(), "bundle:extra".to_owned())],
        "the other bundle's grant survives a prune it was not part of"
    );

    cleanup(&db, &mine).await;
    cleanup(&db, &theirs).await;
}

#[test]
fn a_kind_added_with_no_ids_still_makes_the_scope_a_scope() {
    let scope = IngestScope::new().with_kind(EntityKind::Marketplace, Vec::<String>::new());

    assert!(
        !scope.is_unscoped(),
        "a kind claimed with an empty id list is an assertion of ownership over nothing, not an \
         absence of ownership"
    );
    assert!(
        !scope.owns(EntityKind::Marketplace, "anything-at-all"),
        "a scope that owns no ids owns no id"
    );
}
