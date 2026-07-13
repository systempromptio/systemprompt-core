//! DB-backed behavioural tests for [`AccessControlIngestionService`].
//!
//! These exercise the YAML -> DB projection against the shared fixture
//! database: literal-id and glob-expanded rules, the insert/update/skip
//! outcome accounting, marketplace-access projection (including the
//! empty-roles skip), and the messaging-seed update path. Every test scopes
//! itself to a unique entity id and cleans up on the way out, and skips
//! cleanly when no fixture database is reachable.

use std::collections::HashMap;

use systemprompt_database::DbPool;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{MarketplaceConfig, SlackAppConfig};
use systemprompt_security::authz::{
    AccessControlConfig, AccessControlIngestionService, IngestOptions,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool() -> Option<DbPool> {
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
async fn ingest_config_inserts_updates_and_skips() {
    let Some(db) = pool().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let id = unique_id("ing-route");

    let deny = |access: &str| -> AccessControlConfig {
        serde_yaml::from_str(&format!(
            "rules:\n  - entity_type: gateway_route\n    entity_id: {id}\n    access: {access}\n    \
             roles: [tester]\n"
        ))
        .expect("config yaml parses")
    };

    let inserted = svc
        .ingest_config(&deny("deny"), IngestOptions::default())
        .await
        .expect("first ingest");
    assert_eq!(inserted.inserted, 1, "a fresh grant is inserted");

    let updated = svc
        .ingest_config(
            &deny("allow"),
            IngestOptions {
                override_existing: true,
                delete_orphans: false,
            },
        )
        .await
        .expect("override ingest");
    assert_eq!(
        updated.updated, 1,
        "override of a changed access flips the row to Updated"
    );

    let skipped = svc
        .ingest_config(
            &deny("allow"),
            IngestOptions {
                override_existing: true,
                delete_orphans: false,
            },
        )
        .await
        .expect("no-op ingest");
    assert_eq!(
        skipped.skipped, 1,
        "override of an unchanged row is a Skip, not a rewrite"
    );

    cleanup(&db, "gateway_route", &id).await;
}

#[tokio::test]
async fn ingest_config_expands_entity_match_glob() {
    let Some(db) = pool().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let id = unique_id("ing-glob");

    // Materialise the catalog row via a literal-id rule first, so the glob has
    // something to expand over.
    let literal: AccessControlConfig = serde_yaml::from_str(&format!(
        "rules:\n  - entity_type: gateway_route\n    entity_id: {id}\n    access: allow\n    \
         roles: [seed]\n"
    ))
    .expect("literal yaml");
    svc.ingest_config(&literal, IngestOptions::default())
        .await
        .expect("seed catalog entity");

    let glob: AccessControlConfig = serde_yaml::from_str(&format!(
        "rules:\n  - entity_type: gateway_route\n    entity_match: \"{id}\"\n    access: allow\n    \
         roles: [glob-role]\n"
    ))
    .expect("glob yaml");
    let report = svc
        .ingest_config(&glob, IngestOptions::default())
        .await
        .expect("glob ingest");
    assert_eq!(
        report.inserted, 1,
        "the glob expands to the one materialised catalog id and grants it"
    );

    cleanup(&db, "gateway_route", &id).await;
}

#[tokio::test]
async fn from_pool_constructs_a_usable_service() {
    let Some(db) = pool().await else {
        return;
    };
    let arc = db.write_pool_arc().expect("write pool");
    let svc = AccessControlIngestionService::from_pool(arc);
    let id = unique_id("ing-frompool");
    let cfg: AccessControlConfig = serde_yaml::from_str(&format!(
        "rules:\n  - entity_type: gateway_route\n    entity_id: {id}\n    access: allow\n    \
         roles: [r]\n"
    ))
    .expect("yaml");
    let report = svc
        .ingest_config(&cfg, IngestOptions::default())
        .await
        .expect("from_pool service ingests");
    assert_eq!(report.inserted, 1);

    cleanup(&db, "gateway_route", &id).await;
}

#[tokio::test]
async fn marketplace_with_no_roles_is_skipped_entirely() {
    let Some(db) = pool().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
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
        "a roles-less marketplace writes no rows"
    );
}

#[tokio::test]
async fn slack_seed_updates_an_existing_deny_rule() {
    let Some(db) = pool().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let wsid = unique_id("ws");

    // Seed a deny grant on the workspace entity so the later messaging seed has
    // an existing row to flip to allow.
    let seed: AccessControlConfig = serde_yaml::from_str(&format!(
        "rules:\n  - entity_type: slack_workspace\n    entity_id: {wsid}\n    access: deny\n    \
         roles: [ops]\n"
    ))
    .expect("seed yaml");
    svc.ingest_config(&seed, IngestOptions::default())
        .await
        .expect("seed deny rule");

    let app: SlackAppConfig = serde_yaml::from_str(&format!(
        "workspace_id: {wsid}\nsigning_secret_ref: sig\nbot_token_ref: bot\nenabled: true\n\
         default_agent: assistant\nauthz:\n  allowed_roles: [ops]\n"
    ))
    .expect("slack yaml");
    let mut apps = HashMap::new();
    apps.insert("primary".to_owned(), app);

    let report = svc
        .ingest_slack_apps(
            &apps,
            IngestOptions {
                override_existing: true,
                delete_orphans: false,
            },
        )
        .await
        .expect("slack ingest");
    assert_eq!(
        report.updated, 1,
        "the allow seed overrides the pre-existing deny grant"
    );

    cleanup(&db, "slack_workspace", &wsid).await;
}
