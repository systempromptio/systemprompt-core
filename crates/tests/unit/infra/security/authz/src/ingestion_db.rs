//! DB-backed behavioural tests for [`AccessControlIngestionService`].
//!
//! These exercise the marketplace-access projection against the shared
//! fixture database, including the empty-roles skip. Every test scopes
//! itself to a unique entity id and cleans up on the way out, and skips
//! cleanly when no fixture database is reachable.

use std::collections::HashMap;

use systemprompt_database::DbPool;
use systemprompt_governance::authz::{AccessControlIngestionService, IngestOptions};
use systemprompt_identifiers::MarketplaceId;
use systemprompt_manifest::services::MarketplaceConfig;
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn cleanup(db: &DbPool, entity_type: &str, entity_id: &str) {
    let pg = db.write_pool();
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
async fn marketplace_with_no_roles_is_skipped_entirely() {
    let db = test_db_pool().await;
    let svc = AccessControlIngestionService::new(&db);
    let id = unique_id("mkt");

    let cfg: MarketplaceConfig = serde_yaml::from_str(&format!(
        "id: {id}\nname: Test Market\ndescription: d\nversion: 1.0.0\nlicense: MIT\nauthor:\n  \
         name: t\n  email: t@example.com\naccess:\n  roles: []\n"
    ))
    .expect("marketplace yaml");
    let mut map = HashMap::new();
    map.insert(MarketplaceId::new(&id), cfg);

    let report = svc
        .ingest_marketplace_access(&map, IngestOptions::default())
        .await
        .expect("marketplace ingest");
    assert_eq!(
        (
            report.inserted,
            report.updated,
            report.skipped,
            report.deleted
        ),
        (0, 0, 0, 0),
        "a marketplace declaring neither roles nor rules writes no rows"
    );
}

#[tokio::test]
async fn marketplace_with_only_attribute_rules_is_ingested() {
    let db = test_db_pool().await;
    let svc = AccessControlIngestionService::new(&db);
    let id = unique_id("mkt");

    let cfg: MarketplaceConfig = serde_yaml::from_str(&format!(
        "id: {id}\nname: Test Market\ndescription: d\nversion: 1.0.0\nlicense: MIT\nauthor:\n  \
         name: t\n  email: t@example.com\naccess:\n  roles: []\n  rules:\n    - rule_type: \
         adfs_group\n      values: [commerce-devs, core-devs]\n"
    ))
    .expect("marketplace yaml");
    let mut map = HashMap::new();
    map.insert(MarketplaceId::new(&id), cfg);

    let report = svc
        .ingest_marketplace_access(&map, IngestOptions::default())
        .await
        .expect("marketplace ingest");
    assert_eq!(
        report.inserted, 2,
        "a marketplace with no roles but two group values still projects"
    );

    cleanup(&db, "marketplace", &id).await;
}
