//! Ownership behaviour of the marketplace projection: pruning is scoped to
//! one source inside one ownership scope.

use std::collections::HashMap;

use systemprompt_database::DbPool;
use systemprompt_governance::authz::{
    AccessControlIngestionService, EntityKind, IngestOptions, IngestScope,
};
use systemprompt_identifiers::MarketplaceId;
use systemprompt_manifest::services::MarketplaceConfig;
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

fn options(source: &str, delete_orphans: bool, scope: IngestScope) -> IngestOptions {
    IngestOptions {
        override_existing: true,
        delete_orphans,
        source: source.to_owned(),
        scope,
    }
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

async fn rule_sources(db: &DbPool, entity_id: &str) -> Vec<(String, String, String)> {
    let pg = db.write_pool();
    sqlx::query_as::<_, (String, String, String)>(
        "SELECT rule_type, rule_value, source FROM access_control_rules WHERE entity_id = $1 ORDER \
         BY rule_type, rule_value",
    )
    .bind(entity_id)
    .fetch_all(&*pg)
    .await
    .expect("read back rules")
}

#[tokio::test]
async fn marketplace_prune_is_scoped_to_the_ingesting_source() {
    let db = test_db_pool().await;
    let svc = AccessControlIngestionService::new(&db);
    let id = unique_id("own-mkt");

    let market = |role: &str| -> HashMap<MarketplaceId, MarketplaceConfig> {
        let cfg: MarketplaceConfig = serde_yaml::from_str(&format!(
            "id: {id}\nname: Test Market\ndescription: d\nversion: 1.0.0\nlicense: MIT\nauthor:\n  \
             name: t\n  email: t@example.com\naccess:\n  roles: [{role}]\n"
        ))
        .expect("marketplace yaml");
        let mut map = HashMap::new();
        map.insert(MarketplaceId::new(&id), cfg);
        map
    };

    svc.ingest_marketplace_access(
        &market("stale"),
        options("bundle:b", false, IngestScope::default()),
    )
    .await
    .expect("another bundle grants a role");

    let scope = IngestScope::new().with_kind(EntityKind::Marketplace, [id.clone()]);
    let report = svc
        .ingest_marketplace_access(&market("fresh"), options("bundle:a", true, scope))
        .await
        .expect("this bundle re-ingests with a prune");

    assert_eq!(
        report.deleted, 0,
        "the prune leaves the other bundle's grant on the same band alone"
    );
    let values: Vec<String> = rule_sources(&db, &id)
        .await
        .into_iter()
        .map(|(_, value, source)| format!("{value}@{source}"))
        .collect();
    assert_eq!(
        values,
        vec!["fresh@bundle:a".to_owned(), "stale@bundle:b".to_owned()],
        "both bundles' grants coexist, each carrying its own provenance"
    );

    cleanup(&db, "marketplace", &id).await;
}
