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
    MarketplaceAccess, MarketplaceAccessRule, MarketplaceConfig, MarketplaceRuleAccess,
    MarketplaceVisibility, PluginAuthor, PluginComponentRef,
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
        mcp_servers: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
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
        rules: vec![],
        attributes: Default::default(),
        justification: justification.map(str::to_owned),
    }
}

fn rule(
    rule_type: &str,
    values: &[&str],
    grant: MarketplaceRuleAccess,
) -> MarketplaceAccessRule {
    MarketplaceAccessRule {
        rule_type: rule_type.to_owned(),
        values: values.iter().map(|v| (*v).to_owned()).collect(),
        access: grant,
        justification: None,
    }
}

async fn band_rows(pg: &PgPool, id: &MarketplaceId, rule_type: &str) -> Vec<(String, String)> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT rule_value, access FROM access_control_rules WHERE entity_type='marketplace' AND \
         entity_id=$1 AND rule_type=$2 ORDER BY rule_value",
    )
    .bind(id.as_str())
    .bind(rule_type)
    .fetch_all(pg)
    .await
    .expect("query band rules")
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

#[tokio::test]
async fn attribute_rules_project_to_extension_rule_type_rows() {
    let f = setup().await;
    let service = AccessControlIngestionService::new(&f.db).expect("service");
    let mut block = access(&[], true, None);
    block.rules = vec![rule(
        "adfs_group",
        &["commerce-devs", "core-devs"],
        MarketplaceRuleAccess::Allow,
    )];

    let report = service
        .ingest_marketplace_access(&one(&f.id, block), IngestOptions::default())
        .await
        .expect("ingest");

    assert_eq!(report.inserted, 2);
    let rows = band_rows(&f.pg, &f.id, "adfs_group").await;
    assert_eq!(
        rows,
        vec![
            ("commerce-devs".to_owned(), "allow".to_owned()),
            ("core-devs".to_owned(), "allow".to_owned()),
        ]
    );
    cleanup(&f.pg, &f.id).await;
}

#[tokio::test]
async fn deny_rule_writes_access_deny() {
    let f = setup().await;
    let service = AccessControlIngestionService::new(&f.db).expect("service");
    let mut block = access(&[], true, None);
    block.rules = vec![rule("project", &["storefront"], MarketplaceRuleAccess::Deny)];

    service
        .ingest_marketplace_access(&one(&f.id, block), IngestOptions::default())
        .await
        .expect("ingest");

    assert_eq!(
        band_rows(&f.pg, &f.id, "project").await,
        vec![("storefront".to_owned(), "deny".to_owned())]
    );
    cleanup(&f.pg, &f.id).await;
}

// Why: dropping a band from the YAML must retire exactly that band. A delete
// scoped only by entity would take every rule another writer owns with it.
#[tokio::test]
async fn delete_orphans_only_touches_declared_bands() {
    let f = setup().await;
    let service = AccessControlIngestionService::new(&f.db).expect("service");
    let options = IngestOptions {
        delete_orphans: true,
        ..IngestOptions::default()
    };

    let mut block = access(&["engineer"], true, None);
    block.rules = vec![rule("project", &["storefront"], MarketplaceRuleAccess::Allow)];
    service
        .ingest_marketplace_access(&one(&f.id, block), options)
        .await
        .expect("first ingest");

    let mut narrowed = access(&["engineer"], true, None);
    narrowed.rules = vec![];
    service
        .ingest_marketplace_access(&one(&f.id, narrowed), options)
        .await
        .expect("second ingest");

    assert_eq!(role_values(&f.pg, &f.id).await, vec!["engineer".to_owned()]);
    assert_eq!(
        band_rows(&f.pg, &f.id, "project").await,
        Vec::<(String, String)>::new(),
        "an undeclared band is left in place rather than deleted by this writer",
    );
    cleanup(&f.pg, &f.id).await;
}

#[tokio::test]
async fn invalid_rule_type_slug_is_rejected_before_any_write() {
    let f = setup().await;
    let service = AccessControlIngestionService::new(&f.db).expect("service");
    let mut block = access(&["engineer"], true, None);
    block.rules = vec![rule("Bad-Slug", &["x"], MarketplaceRuleAccess::Allow)];

    let err = service
        .ingest_marketplace_access(&one(&f.id, block), IngestOptions::default())
        .await
        .expect_err("a malformed subject dimension must not be written");
    assert!(err.to_string().contains("Bad-Slug"), "{err}");
    assert!(
        role_values(&f.pg, &f.id).await.is_empty(),
        "no partial write survives the rejection",
    );
    cleanup(&f.pg, &f.id).await;
}
