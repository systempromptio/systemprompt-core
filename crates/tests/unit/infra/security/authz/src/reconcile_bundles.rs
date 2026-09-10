//! Two bundles composed into one root: each owns its own marketplaces, and
//! neither prune reaches the other's rows.
//!
//! A bundle that owns nothing is the dangerous edge. Its scope must still be a
//! scope: an ownership set that collapses to "unscoped" would turn the bundle
//! that claims least into the one that prunes most.

use std::collections::HashMap;

use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{
    BundleOwnership, BundleSourceInfo, MarketplaceConfig, ServicesBundleManifest, ServicesConfig,
};
use systemprompt_security::authz::{EntityKind, IngestScope, reconcile_composed_bundles};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool_or_skip() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

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
    let pg = db.write_pool_arc().expect("write pool");
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
    let pg = db.write_pool_arc().expect("write pool");
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
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let base_market = unique_id("bun-base");
    let extra_market = unique_id("bun-extra");
    let root = tempfile::tempdir().expect("composed root");

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

    let reports = reconcile_composed_bundles(&db, &services, root.path(), &bundles)
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
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let mine = unique_id("bun-mine");
    let theirs = unique_id("bun-theirs");
    let root = tempfile::tempdir().expect("composed root");

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

    reconcile_composed_bundles(
        &db,
        &services_with("stale", "keepme"),
        root.path(),
        &bundles,
    )
    .await
    .expect("seed both bundles");

    let only_base = [("base", &base)];
    reconcile_composed_bundles(
        &db,
        &services_with("fresh", "keepme"),
        root.path(),
        &only_base,
    )
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

#[tokio::test]
async fn a_bundle_that_owns_nothing_prunes_nothing() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let route = unique_id("bun-empty-route");
    let theirs = unique_id("bun-empty-theirs");
    let root = tempfile::tempdir().expect("composed root");

    std::fs::create_dir_all(root.path().join("access-control")).expect("access-control dir");
    std::fs::write(
        root.path().join("access-control/roles.yaml"),
        format!(
            "rules:\n  - entity_type: gateway_route\n    entity_id: {route}\n    access: allow\n    roles: [keep]\n"
        ),
    )
    .expect("write roles.yaml");

    let mut services = ServicesConfig::default();
    let mut markets = HashMap::new();
    let (id, cfg) = marketplace(&theirs, "keepme");
    markets.insert(id, cfg);
    services.marketplaces = markets;

    let nothing = manifest(Vec::new());
    let owner = manifest(vec![theirs.clone()]);
    let bundles = [("base", &nothing), ("extra", &owner)];

    let first = reconcile_composed_bundles(&db, &services, root.path(), &bundles)
        .await
        .expect("first composition");
    assert_eq!(
        first[0].1.roles.as_ref().expect("roles pass ran").inserted,
        1,
        "the base bundle still projects roles.yaml while owning nothing"
    );
    assert_eq!(
        first[1]
            .1
            .marketplaces
            .as_ref()
            .expect("marketplace pass ran")
            .inserted,
        1,
        "the owning bundle projects the marketplace it claims"
    );

    let second = reconcile_composed_bundles(&db, &services, root.path(), &bundles)
        .await
        .expect("second composition");
    let roles = second[0].1.roles.as_ref().expect("roles pass ran");
    assert_eq!(
        (roles.deleted, roles.skipped),
        (0, 1),
        "an all-empty ownership set prunes nothing, not everything its own source wrote"
    );
    assert_eq!(
        second[0]
            .1
            .marketplaces
            .as_ref()
            .expect("marketplace pass ran")
            .deleted,
        0,
        "and it prunes no marketplace band either"
    );
    assert_eq!(
        rules_for(&db, &theirs).await,
        vec![("keepme".to_owned(), "bundle:extra".to_owned())],
        "the owning bundle's grant is untouched by a bundle that owns nothing"
    );
    assert_eq!(
        rules_for(&db, &route).await,
        vec![("keep".to_owned(), "bundle:base".to_owned())],
        "the roles.yaml grant stays owned by the bundle that carries access-control"
    );

    cleanup(&db, &route).await;
    cleanup(&db, &theirs).await;
}
