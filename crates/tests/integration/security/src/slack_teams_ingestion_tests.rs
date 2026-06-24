//! DB-backed coverage for [`AccessControlIngestionService::ingest_slack_apps`]
//! and [`AccessControlIngestionService::ingest_teams_apps`].
//!
//! Each test scopes itself to a unique workspace/tenant id so concurrent runs
//! against the shared `DATABASE_URL` never collide, and cleans up its rows.
//! Mirrors `marketplace_ingestion_tests.rs`.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, SecretName, SlackWorkspaceId, TeamsTenantId};
use systemprompt_models::services::{
    SlackAppConfig, SlackAuthzConfig, TeamsAppConfig, TeamsAuthzConfig,
};
use systemprompt_security::authz::{AccessControlIngestionService, IngestOptions};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

struct Fixture {
    db: DbPool,
    pg: Arc<PgPool>,
}

async fn setup() -> Fixture {
    let url = fixture_database_url().expect("DATABASE_URL");
    let db = fixture_db_pool(&url).await.expect("connect test database");
    let pg = db.pool_arc().expect("read pool");
    Fixture { db, pg }
}

fn slack_id() -> String {
    format!("T_TEST_{}", Uuid::new_v4().simple())
}

fn teams_id() -> String {
    format!("tenant-test-{}", Uuid::new_v4().simple())
}

async fn cleanup(pg: &PgPool, entity_type: &str, id: &str) {
    sqlx::query("DELETE FROM access_control_rules WHERE entity_type=$1 AND entity_id=$2")
        .bind(entity_type)
        .bind(id)
        .execute(pg)
        .await
        .expect("cleanup rules");
    sqlx::query("DELETE FROM access_control_entities WHERE entity_type=$1 AND entity_id=$2")
        .bind(entity_type)
        .bind(id)
        .execute(pg)
        .await
        .expect("cleanup entities");
}

fn slack_app(workspace: &str, roles: &[&str], enabled: bool) -> SlackAppConfig {
    SlackAppConfig {
        workspace_id: SlackWorkspaceId::new(workspace),
        signing_secret_ref: SecretName::new("slack_signing_secret"),
        bot_token_ref: SecretName::new("slack_bot_token"),
        enabled,
        default_agent: Some(AgentName::new("test_agent")),
        routing: BTreeMap::new(),
        authz: SlackAuthzConfig {
            allowed_roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        },
    }
}

fn teams_app(tenant: &str, roles: &[&str], enabled: bool) -> TeamsAppConfig {
    TeamsAppConfig {
        tenant_id: TeamsTenantId::new(tenant),
        app_id: "app-test".to_owned(),
        app_password_ref: SecretName::new("teams_app_password"),
        enabled,
        default_agent: Some(AgentName::new("test_agent")),
        routing: BTreeMap::new(),
        authz: TeamsAuthzConfig {
            allowed_roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        },
    }
}

fn slack_map(app: SlackAppConfig) -> HashMap<String, SlackAppConfig> {
    let mut map = HashMap::new();
    map.insert("app".to_owned(), app);
    map
}

fn teams_map(app: TeamsAppConfig) -> HashMap<String, TeamsAppConfig> {
    let mut map = HashMap::new();
    map.insert("app".to_owned(), app);
    map
}

async fn role_values(pg: &PgPool, entity_type: &str, id: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT rule_value FROM access_control_rules WHERE entity_type=$1 AND entity_id=$2 AND \
         rule_type='role' ORDER BY rule_value",
    )
    .bind(entity_type)
    .bind(id)
    .fetch_all(pg)
    .await
    .expect("query role rules")
}

#[tokio::test]
async fn slack_happy_path_projects_entity_and_rules() {
    let f = setup().await;
    let id = slack_id();
    let service = AccessControlIngestionService::new(&f.db).expect("service");

    let report = service
        .ingest_slack_apps(
            &slack_map(slack_app(&id, &["engineer", "admin"], true)),
            IngestOptions::default(),
        )
        .await
        .expect("ingest");
    assert_eq!(report.inserted, 2, "two role rules inserted");

    let default_included: bool = sqlx::query_scalar(
        "SELECT default_included FROM access_control_entities WHERE entity_type='slack_workspace' \
         AND entity_id=$1",
    )
    .bind(&id)
    .fetch_one(f.pg.as_ref())
    .await
    .expect("entity row exists");
    assert!(
        !default_included,
        "workspace entity is not default-included"
    );

    assert_eq!(
        role_values(&f.pg, "slack_workspace", &id).await,
        vec!["admin", "engineer"]
    );

    let accesses: Vec<String> = sqlx::query_scalar(
        "SELECT access FROM access_control_rules WHERE entity_type='slack_workspace' AND \
         entity_id=$1",
    )
    .bind(&id)
    .fetch_all(f.pg.as_ref())
    .await
    .expect("query access");
    assert!(
        accesses.iter().all(|a| a == "allow"),
        "role grants are allow"
    );

    cleanup(&f.pg, "slack_workspace", &id).await;
}

#[tokio::test]
async fn teams_happy_path_projects_entity_and_rules() {
    let f = setup().await;
    let id = teams_id();
    let service = AccessControlIngestionService::new(&f.db).expect("service");

    let report = service
        .ingest_teams_apps(
            &teams_map(teams_app(&id, &["user"], true)),
            IngestOptions::default(),
        )
        .await
        .expect("ingest");
    assert_eq!(report.inserted, 1);
    assert_eq!(role_values(&f.pg, "teams_tenant", &id).await, vec!["user"]);

    cleanup(&f.pg, "teams_tenant", &id).await;
}

#[tokio::test]
async fn disabled_and_empty_role_apps_produce_no_rows() {
    let f = setup().await;
    let disabled = slack_id();
    let empty = slack_id();
    let service = AccessControlIngestionService::new(&f.db).expect("service");

    let mut apps = HashMap::new();
    apps.insert(
        "disabled".to_owned(),
        slack_app(&disabled, &["admin"], false),
    );
    apps.insert("empty".to_owned(), slack_app(&empty, &[], true));

    let report = service
        .ingest_slack_apps(&apps, IngestOptions::default())
        .await
        .expect("ingest");
    assert_eq!(report.inserted, 0, "neither app contributes rules");

    assert!(
        role_values(&f.pg, "slack_workspace", &disabled)
            .await
            .is_empty()
    );
    assert!(
        role_values(&f.pg, "slack_workspace", &empty)
            .await
            .is_empty()
    );

    cleanup(&f.pg, "slack_workspace", &disabled).await;
    cleanup(&f.pg, "slack_workspace", &empty).await;
}

#[tokio::test]
async fn delete_orphans_is_scoped_to_the_ingested_ids() {
    let f = setup().await;
    let kept = slack_id();
    let swept = slack_id();
    let service = AccessControlIngestionService::new(&f.db).expect("service");

    // First pass seeds both workspaces.
    let mut both = HashMap::new();
    both.insert("kept".to_owned(), slack_app(&kept, &["admin"], true));
    both.insert("swept".to_owned(), slack_app(&swept, &["engineer"], true));
    service
        .ingest_slack_apps(&both, IngestOptions::default())
        .await
        .expect("first ingest");

    // Second pass re-ingests only `swept` with delete_orphans — the sweep is
    // scoped to its id, so `kept`'s foreign role rule survives untouched.
    service
        .ingest_slack_apps(
            &slack_map(slack_app(&swept, &["engineer"], true)),
            IngestOptions {
                override_existing: true,
                delete_orphans: true,
            },
        )
        .await
        .expect("second ingest");

    assert_eq!(
        role_values(&f.pg, "slack_workspace", &kept).await,
        vec!["admin"],
        "a workspace outside the pass keeps its rules"
    );
    assert_eq!(
        role_values(&f.pg, "slack_workspace", &swept).await,
        vec!["engineer"]
    );

    cleanup(&f.pg, "slack_workspace", &kept).await;
    cleanup(&f.pg, "slack_workspace", &swept).await;
}

#[tokio::test]
async fn re_ingest_without_override_is_idempotent() {
    let f = setup().await;
    let id = slack_id();
    let service = AccessControlIngestionService::new(&f.db).expect("service");

    service
        .ingest_slack_apps(
            &slack_map(slack_app(&id, &["engineer"], true)),
            IngestOptions::default(),
        )
        .await
        .expect("first ingest");

    let again = service
        .ingest_slack_apps(
            &slack_map(slack_app(&id, &["engineer"], true)),
            IngestOptions::default(),
        )
        .await
        .expect("second ingest without override");
    assert_eq!(
        again.skipped, 1,
        "no override leaves the existing rule untouched"
    );
    assert_eq!(again.inserted, 0);

    assert_eq!(
        role_values(&f.pg, "slack_workspace", &id).await,
        vec!["engineer"]
    );

    cleanup(&f.pg, "slack_workspace", &id).await;
}
