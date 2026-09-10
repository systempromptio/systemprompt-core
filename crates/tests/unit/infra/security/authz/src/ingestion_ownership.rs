//! Ownership behaviour of the rule projection: dashboard rows are protected,
//! pruning is scoped to one source inside one ownership scope, and a role no
//! user holds is reported back to the caller.

use std::collections::HashMap;

use systemprompt_database::DbPool;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::MarketplaceConfig;
use systemprompt_security::authz::{
    Access, AccessControlConfig, AccessControlIngestionService, AccessControlRepository,
    DASHBOARD_SOURCE, EntityKind, IngestOptions, IngestScope, RegisteredEntities, RuleType,
    UpsertRuleParams, YAML_SOURCE,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool_or_skip() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

fn route_config(id: &str, role: &str, access: &str) -> AccessControlConfig {
    serde_yaml::from_str(&format!(
        "rules:\n  - entity_type: gateway_route\n    entity_id: {id}\n    access: {access}\n    \
         roles: [{role}]\n"
    ))
    .expect("config yaml parses")
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

async fn rule_sources(db: &DbPool, entity_id: &str) -> Vec<(String, String, String)> {
    let pg = db.write_pool_arc().expect("write pool");
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
async fn dashboard_rule_survives_an_overriding_ingest() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let repo = AccessControlRepository::new(&db).expect("repository");
    let id = unique_id("own-dash");

    svc.ingest_config(
        &route_config(&id, "seed", "allow"),
        options(YAML_SOURCE, false, IngestScope::default()),
        &RegisteredEntities::default(),
    )
    .await
    .expect("seed the catalog entity");

    repo.upsert_rule(UpsertRuleParams {
        entity_type: EntityKind::GatewayRoute,
        entity_id: &id,
        rule_type: RuleType::ROLE,
        rule_value: "operator",
        access: Access::Deny,
        justification: Some("hand-authored in the dashboard"),
        source: DASHBOARD_SOURCE,
    })
    .await
    .expect("dashboard rule");

    let report = svc
        .ingest_config(
            &route_config(&id, "operator", "allow"),
            options(YAML_SOURCE, false, IngestScope::default()),
            &RegisteredEntities::default(),
        )
        .await
        .expect("ingest over the dashboard rule");

    assert_eq!(
        (report.protected, report.updated),
        (1, 0),
        "an override ingest reports the dashboard row as protected and rewrites nothing"
    );

    let rules = rule_sources(&db, &id).await;
    let operator = rules
        .iter()
        .find(|(_, value, _)| value == "operator")
        .expect("the dashboard rule is still there");
    assert_eq!(
        operator.2, DASHBOARD_SOURCE,
        "the dashboard row keeps its provenance"
    );

    cleanup(&db, "gateway_route", &id).await;
}

#[tokio::test]
async fn prune_takes_only_its_own_source_inside_its_own_scope() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let repo = AccessControlRepository::new(&db).expect("repository");
    let mine = unique_id("own-mine");
    let theirs = unique_id("own-theirs");

    svc.ingest_config(
        &route_config(&mine, "stale", "allow"),
        options("bundle:a", false, IngestScope::default()),
        &RegisteredEntities::default(),
    )
    .await
    .expect("bundle a seeds its own rule");
    svc.ingest_config(
        &route_config(&theirs, "other", "allow"),
        options("bundle:b", false, IngestScope::default()),
        &RegisteredEntities::default(),
    )
    .await
    .expect("bundle b seeds its own rule");

    for (rule_type, value, source) in [
        (RuleType::ROLE, "operator", DASHBOARD_SOURCE),
        (RuleType::USER, "user-42", DASHBOARD_SOURCE),
    ] {
        repo.upsert_rule(UpsertRuleParams {
            entity_type: EntityKind::GatewayRoute,
            entity_id: &mine,
            rule_type,
            rule_value: value,
            access: Access::Allow,
            justification: None,
            source,
        })
        .await
        .expect("operator-authored rule");
    }

    let scope = IngestScope::new().with_kind(EntityKind::GatewayRoute, [mine.clone()]);
    let report = svc
        .ingest_config(
            &route_config(&mine, "fresh", "allow"),
            options("bundle:a", true, scope),
            &RegisteredEntities::default(),
        )
        .await
        .expect("bundle a re-ingests with a prune");

    assert_eq!(
        report.deleted, 1,
        "only bundle a's own stale role rule goes"
    );

    let surviving: Vec<String> = rule_sources(&db, &mine)
        .await
        .into_iter()
        .map(|(rule_type, value, _)| format!("{rule_type}:{value}"))
        .collect();
    assert_eq!(
        surviving,
        vec![
            "role:fresh".to_owned(),
            "role:operator".to_owned(),
            "user:user-42".to_owned()
        ],
        "the dashboard role rule and the per-user override are untouched by the prune"
    );
    assert_eq!(
        rule_sources(&db, &theirs).await.len(),
        1,
        "another bundle's rule on another entity is untouched"
    );

    cleanup(&db, "gateway_route", &mine).await;
    cleanup(&db, "gateway_route", &theirs).await;
}

#[tokio::test]
async fn prune_skips_an_entity_outside_the_ownership_scope() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let id = unique_id("own-unowned");

    svc.ingest_config(
        &route_config(&id, "stale", "allow"),
        options("bundle:a", false, IngestScope::default()),
        &RegisteredEntities::default(),
    )
    .await
    .expect("seed");

    let scope = IngestScope::new().with_kind(EntityKind::GatewayRoute, ["some-other-route"]);
    let report = svc
        .ingest_config(
            &route_config(&id, "fresh", "allow"),
            options("bundle:a", true, scope),
            &RegisteredEntities::default(),
        )
        .await
        .expect("ingest with a prune that owns nothing here");

    assert_eq!(
        report.deleted, 0,
        "an entity the scope does not claim is never pruned, source match or not"
    );
    assert_eq!(
        rule_sources(&db, &id).await.len(),
        2,
        "both the stale and the fresh grant remain"
    );

    cleanup(&db, "gateway_route", &id).await;
}

#[tokio::test]
async fn a_role_nobody_holds_is_reported_and_a_held_one_is_not() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
    let id = unique_id("own-subject");
    let held = unique_id("role-held").replace('-', "_");
    let ghost = unique_id("role-ghost").replace('-', "_");
    let user_id = unique_id("subject-user");

    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query("INSERT INTO users (id, name, email, roles) VALUES ($1, $2, $3, ARRAY[$4])")
        .bind(&user_id)
        .bind("subject fixture")
        .bind(format!("{user_id}@example.com"))
        .bind(&held)
        .execute(&*pg)
        .await
        .expect("insert a user holding the role");

    let cfg: AccessControlConfig = serde_yaml::from_str(&format!(
        "rules:\n  - entity_type: gateway_route\n    entity_id: {id}\n    access: allow\n    \
         roles: [{held}, {ghost}]\n"
    ))
    .expect("config yaml parses");

    let report = svc
        .ingest_config(
            &cfg,
            options(YAML_SOURCE, false, IngestScope::default()),
            &RegisteredEntities::default(),
        )
        .await
        .expect("ingest");

    let reported: Vec<&str> = report
        .unknown_subjects
        .iter()
        .map(|subject| subject.value.as_str())
        .collect();
    assert_eq!(
        reported,
        vec![ghost.as_str()],
        "only the role no user holds is reported back"
    );
    assert_eq!(report.unknown_subjects[0].rule_type, "role");
    assert_eq!(
        report.unknown_subjects[0].entity,
        format!("gateway_route:{id}")
    );

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&user_id)
        .execute(&*pg)
        .await
        .expect("cleanup user");
    cleanup(&db, "gateway_route", &id).await;
}

#[tokio::test]
async fn marketplace_prune_is_scoped_to_the_ingesting_source() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let svc = AccessControlIngestionService::new(&db).expect("ingestion service");
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
