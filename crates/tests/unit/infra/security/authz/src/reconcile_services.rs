//! End-to-end projection of a services tree by `reconcile_services_authz`.

use std::collections::HashMap;

use systemprompt_database::DbPool;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{MarketplaceConfig, ServicesConfig};
use systemprompt_security::authz::{IngestScope, reconcile_services_authz};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool_or_skip() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn cleanup(db: &DbPool, entity_type: &str, entity_id: &str) {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM access_control_rules WHERE entity_type = $1 AND entity_id = $2")
        .bind(entity_type)
        .bind(entity_id)
        .execute(&*pg)
        .await
        .expect("cleanup rules");
    sqlx::query("DELETE FROM access_control_entities WHERE entity_type = $1 AND entity_id = $2")
        .bind(entity_type)
        .bind(entity_id)
        .execute(&*pg)
        .await
        .expect("cleanup entities");
}

#[tokio::test]
async fn reconcile_projects_roles_yaml_and_marketplace_access() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let route = unique_id("rec-route");
    let market = unique_id("rec-mkt");

    let root = tempfile::tempdir().expect("temp services root");
    std::fs::create_dir_all(root.path().join("access-control")).expect("access-control dir");
    std::fs::write(
        root.path().join("access-control/roles.yaml"),
        format!(
            "rules:\n  - entity_type: gateway_route\n    entity_id: {route}\n    access: allow\n \
             \n    roles: [ops]\n"
        ),
    )
    .expect("write roles.yaml");

    let mut services = ServicesConfig::default();
    let cfg: MarketplaceConfig = serde_yaml::from_str(&format!(
        "id: {market}\nname: Test Market\ndescription: d\nversion: 1.0.0\nlicense: MIT\nauthor:\n  \
         name: t\n  email: t@example.com\naccess:\n  roles: [ops]\n"
    ))
    .expect("marketplace yaml");
    let mut markets = HashMap::new();
    markets.insert(MarketplaceId::new(&market), cfg);
    services.marketplaces = markets;

    let scope = IngestScope::new().with_kind(
        systemprompt_security::authz::EntityKind::Marketplace,
        [market.clone()],
    );
    let report = reconcile_services_authz(
        &db,
        &services,
        root.path(),
        "bundle:test",
        Some(scope.clone()),
    )
    .await
    .expect("reconcile");

    assert!(
        report.gateway.is_none(),
        "a tree with no gateway declares no routes to reconcile"
    );
    assert_eq!(
        report.roles.expect("roles.yaml was projected").inserted,
        1,
        "the roles.yaml grant is inserted"
    );
    assert_eq!(
        report
            .marketplaces
            .expect("marketplace access was projected")
            .inserted,
        1,
        "the marketplace access block is projected"
    );

    let again = reconcile_services_authz(&db, &services, root.path(), "bundle:test", Some(scope))
        .await
        .expect("second reconcile");
    assert_eq!(
        (
            again.roles.expect("roles").skipped,
            again.marketplaces.expect("marketplaces").skipped
        ),
        (1, 1),
        "reconciling an unchanged tree rewrites nothing"
    );

    cleanup(&db, "gateway_route", &route).await;
    cleanup(&db, "marketplace", &market).await;
}
