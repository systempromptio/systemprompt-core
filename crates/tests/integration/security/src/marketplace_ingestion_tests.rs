//! DB-backed coverage for
//! [`AccessControlIngestionService::ingest_marketplace_access`].
//!
//! Each test scopes itself to a unique marketplace id so concurrent runs
//! against the shared `DATABASE_URL` never collide, and cleans up its rows.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{
    MarketplaceAccess, MarketplaceConfig, MarketplaceVisibility, PluginAuthor, PluginComponentRef,
};
use systemprompt_security::authz::{AccessControlIngestionService, IngestOptions};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

struct Fixture {
    db: DbPool,
    pg: Arc<PgPool>,
    id: MarketplaceId,
}

async fn setup() -> Fixture {
    let url = fixture_database_url().expect("DATABASE_URL");
    let db = fixture_db_pool(&url).await.expect("connect test database");
    let pg = db.pool_arc().expect("read pool");
    let id = MarketplaceId::new(format!("mp-test-{}", Uuid::new_v4()));
    cleanup(&pg, &id).await;
    Fixture { db, pg, id }
}

async fn cleanup(pg: &PgPool, id: &MarketplaceId) {
    sqlx::query(
        "DELETE FROM access_control_rules WHERE entity_type='marketplace' AND entity_id=$1",
    )
    .bind(id.as_str())
    .execute(pg)
    .await
    .expect("cleanup rules");
    sqlx::query(
        "DELETE FROM access_control_entities WHERE entity_type='marketplace' AND entity_id=$1",
    )
    .bind(id.as_str())
    .execute(pg)
    .await
    .expect("cleanup entities");
}

fn marketplace(id: &MarketplaceId, access: MarketplaceAccess) -> MarketplaceConfig {
    MarketplaceConfig {
        id: id.clone(),
        name: "Test".to_owned(),
        description: "Test marketplace".to_owned(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: PluginAuthor {
            name: "test".to_owned(),
            email: "test@example.com".to_owned(),
        },
        keywords: vec![],
        license: "MIT".to_owned(),
        visibility: MarketplaceVisibility::Public,
        plugins: PluginComponentRef::default(),
        skills: PluginComponentRef::default(),
        mcp_servers: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        access,
    }
}

fn access(
    roles: &[&str],
    default_included: bool,
    justification: Option<&str>,
) -> MarketplaceAccess {
    MarketplaceAccess {
        default_included,
        roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        attributes: Default::default(),
        justification: justification.map(str::to_owned),
    }
}

fn one(id: &MarketplaceId, access: MarketplaceAccess) -> HashMap<MarketplaceId, MarketplaceConfig> {
    let mut map = HashMap::new();
    map.insert(id.clone(), marketplace(id, access));
    map
}

async fn role_values(pg: &PgPool, id: &MarketplaceId) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT rule_value FROM access_control_rules WHERE entity_type='marketplace' AND \
         entity_id=$1 AND rule_type='role' ORDER BY rule_value",
    )
    .bind(id.as_str())
    .fetch_all(pg)
    .await
    .expect("query role rules")
}

#[tokio::test]
async fn happy_path_projects_entity_and_rules() {
    let f = setup().await;
    let service = AccessControlIngestionService::new(&f.db).expect("service");

    let report = service
        .ingest_marketplace_access(
            &one(
                &f.id,
                access(&["engineer", "admin"], true, Some("governance")),
            ),
            IngestOptions::default(),
        )
        .await
        .expect("ingest");

    assert_eq!(report.inserted, 2, "two role rules inserted");

    let default_included: bool = sqlx::query_scalar(
        "SELECT default_included FROM access_control_entities WHERE entity_type='marketplace' AND \
         entity_id=$1",
    )
    .bind(f.id.as_str())
    .fetch_one(f.pg.as_ref())
    .await
    .expect("entity row exists");
    assert!(default_included, "entity carries the YAML default_included");

    assert_eq!(role_values(&f.pg, &f.id).await, vec!["admin", "engineer"]);

    let accesses: Vec<String> = sqlx::query_scalar(
        "SELECT access FROM access_control_rules WHERE entity_type='marketplace' AND entity_id=$1",
    )
    .bind(f.id.as_str())
    .fetch_all(f.pg.as_ref())
    .await
    .expect("query access");
    assert!(
        accesses.iter().all(|a| a == "allow"),
        "role grants are allow"
    );

    cleanup(&f.pg, &f.id).await;
}

#[tokio::test]
async fn delete_orphans_drops_roles_absent_from_the_new_pass() {
    let f = setup().await;
    let service = AccessControlIngestionService::new(&f.db).expect("service");

    service
        .ingest_marketplace_access(
            &one(&f.id, access(&["engineer", "contractor"], false, None)),
            IngestOptions::default(),
        )
        .await
        .expect("first ingest");
    assert_eq!(
        role_values(&f.pg, &f.id).await,
        vec!["contractor", "engineer"]
    );

    let report = service
        .ingest_marketplace_access(
            &one(&f.id, access(&["engineer"], false, None)),
            IngestOptions {
                override_existing: true,
                delete_orphans: true,
            },
        )
        .await
        .expect("second ingest");

    assert_eq!(report.deleted, 2, "prior role rules swept before re-insert");
    assert_eq!(report.inserted, 1, "surviving role re-inserted");
    assert_eq!(role_values(&f.pg, &f.id).await, vec!["engineer"]);

    cleanup(&f.pg, &f.id).await;
}

#[tokio::test]
async fn override_existing_updates_justification() {
    let f = setup().await;
    let service = AccessControlIngestionService::new(&f.db).expect("service");

    service
        .ingest_marketplace_access(
            &one(&f.id, access(&["engineer"], false, Some("first"))),
            IngestOptions::default(),
        )
        .await
        .expect("first ingest");

    let skipped = service
        .ingest_marketplace_access(
            &one(&f.id, access(&["engineer"], false, Some("second"))),
            IngestOptions::default(),
        )
        .await
        .expect("second ingest without override");
    assert_eq!(skipped.skipped, 1, "no override leaves the rule untouched");

    let updated = service
        .ingest_marketplace_access(
            &one(&f.id, access(&["engineer"], false, Some("second"))),
            IngestOptions {
                override_existing: true,
                delete_orphans: false,
            },
        )
        .await
        .expect("third ingest with override");
    assert_eq!(updated.updated, 1, "override rewrites the changed rule");

    let justification: Option<String> = sqlx::query_scalar(
        "SELECT justification FROM access_control_rules WHERE entity_type='marketplace' AND \
         entity_id=$1 AND rule_value='engineer'",
    )
    .bind(f.id.as_str())
    .fetch_one(f.pg.as_ref())
    .await
    .expect("rule exists");
    assert_eq!(justification.as_deref(), Some("second"));

    cleanup(&f.pg, &f.id).await;
}
