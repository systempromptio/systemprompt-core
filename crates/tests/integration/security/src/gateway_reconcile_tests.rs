//! DB-backed coverage for gateway-route entity reconciliation and the wildcard
//! grant that authorizes the *synthesized* catch-all route.
//!
//! This is the end-to-end proof that the unit invariants
//! (`dispatchable_route_ids_*`) cannot give: that `reconcile_gateway_entities`
//! actually materializes a content-addressed `star-*` id into
//! `access_control_entities`, that a `entity_match: "*"` rule expands onto that
//! code-synthesized id (closing the implicit YAML-vs-code coupling), and that
//! the resolver then allows a granted role while still denying an id that has
//! no catalog row (`UnknownEntity`, fail-closed).
//!
//! Each test scopes itself to a unique provider/default-provider so concurrent
//! runs against the shared `DATABASE_URL` never collide, and cleans up its
//! rows.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName, UserId};
use systemprompt_models::profile::{
    ApiSurface, GatewayConfig, ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol,
    synthesize_route_id,
};
use systemprompt_security::authz::{
    Access, AccessControlConfig, AccessControlIngestionService, AccessControlRepository, Decision,
    DenyReason, EntityKind, EntityRef, IngestOptions, ResolveInput, RuleEntry, RuleTarget,
    reconcile_gateway_entities, resolve,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

struct Fixture {
    db: DbPool,
    pg: Arc<PgPool>,
    provider: String,
    default_route_id: RouteId,
}

async fn setup() -> Fixture {
    let url = fixture_database_url().expect("DATABASE_URL");
    let db = fixture_db_pool(&url).await.expect("connect test database");
    let pg = db.pool_arc().expect("read pool");
    // A unique provider name keeps the synthesized catch-all id (`star-<hash>`)
    // distinct from every other test and from the live profile's routes.
    let provider = format!("recon-{}", Uuid::new_v4().simple());
    let default_route_id = synthesize_route_id("*", &provider);
    cleanup(&pg, &default_route_id).await;
    Fixture {
        db,
        pg,
        provider,
        default_route_id,
    }
}

async fn cleanup(pg: &PgPool, id: &RouteId) {
    sqlx::query(
        "DELETE FROM access_control_rules WHERE entity_type='gateway_route' AND entity_id=$1",
    )
    .bind(id.as_str())
    .execute(pg)
    .await
    .expect("cleanup rules");
    sqlx::query(
        "DELETE FROM access_control_entities WHERE entity_type='gateway_route' AND entity_id=$1",
    )
    .bind(id.as_str())
    .execute(pg)
    .await
    .expect("cleanup entities");
}

fn registry(name: &str) -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new(name),
            wire: WireProtocol::Anthropic,
            surface: ApiSurface::Anthropic,
            endpoint: "https://example.test/v1".to_owned(),
            api_key_secret: SecretName::new("test"),
            extra_headers: HashMap::new(),
            models: vec![ProviderModel {
                id: ModelId::new("any"),
                aliases: Vec::new(),
                upstream_model: None,
                pricing: Default::default(),
                capabilities: Default::default(),
                limits: Default::default(),
            }],
        }],
    }
}

fn gateway_with_default(provider: &str) -> GatewayConfig {
    GatewayConfig {
        enabled: true,
        default_provider: Some(ProviderId::new(provider)),
        ..GatewayConfig::default()
    }
}

fn wildcard_gateway_rule(roles: &[&str]) -> AccessControlConfig {
    AccessControlConfig {
        rules: vec![RuleEntry {
            entity_type: EntityKind::GatewayRoute,
            target: RuleTarget::Match("*".to_owned()),
            access: Access::Allow,
            default_included: true,
            roles: roles.iter().map(|r| (*r).to_owned()).collect(),
            justification: None,
        }],
    }
}

async fn role_values(pg: &PgPool, id: &RouteId) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT rule_value FROM access_control_rules WHERE entity_type='gateway_route' AND \
         entity_id=$1 AND rule_type='role' ORDER BY rule_value",
    )
    .bind(id.as_str())
    .fetch_all(pg)
    .await
    .expect("query role rules")
}

#[tokio::test]
async fn reconcile_materializes_synthetic_default_route_and_wildcard_grants_it() {
    let f = setup().await;
    let repo = AccessControlRepository::new(&f.db).expect("repo");

    // 1. Reconcile the profile's dispatchable routes (here: only the synthetic
    //    default catch-all) into the entity catalog.
    let gateway = gateway_with_default(&f.provider);
    let registry = registry(&f.provider);
    let ids = gateway.dispatchable_route_ids(&registry);
    assert!(
        ids.contains(&f.default_route_id),
        "dispatchable ids must include the synthesized catch-all {}",
        f.default_route_id.as_str()
    );
    let id_refs: Vec<&str> = ids.iter().map(RouteId::as_str).collect();
    reconcile_gateway_entities(&repo, &id_refs, "test:gateway_reconcile")
        .await
        .expect("reconcile");

    // Entity row exists, registered default_included=false: presence in the
    // catalog never grants on its own.
    let entity = repo
        .get_entity(EntityKind::GatewayRoute, f.default_route_id.as_str())
        .await
        .expect("get_entity")
        .expect("synthetic route entity materialized");
    assert!(
        !entity.default_included,
        "reconcile registers gateway routes default_included=false"
    );

    // 2. Ingest the wildcard rule; it must expand onto the synthesized id.
    let service = AccessControlIngestionService::new(&f.db).expect("service");
    service
        .ingest_config(
            &wildcard_gateway_rule(&["user", "admin"]),
            IngestOptions::default(),
        )
        .await
        .expect("ingest wildcard");
    assert_eq!(
        role_values(&f.pg, &f.default_route_id).await,
        vec!["admin", "user"],
        "entity_match: \"*\" must grant the code-synthesized route id"
    );

    // 3. Resolver: an admin is allowed on the synthesized route…
    let admin = UserId::new("u-admin");
    let entity_ref = EntityRef::GatewayRoute(f.default_route_id.clone());
    let rules = repo
        .list_rules_for_entity(EntityKind::GatewayRoute, f.default_route_id.as_str())
        .await
        .expect("list rules");
    let decision = resolve(ResolveInput {
        entity: &entity_ref,
        rules: &rules,
        user_id: &admin,
        user_roles: &["admin".to_owned()],
        default_included: Some(entity.default_included),
        parents: &[],
    });
    assert!(
        matches!(decision, Decision::Allow { .. }),
        "admin must be allowed on the granted synthetic route, got {decision:?}"
    );

    // …while an id with no catalog row stays fail-closed (UnknownEntity).
    let bogus = RouteId::new(format!("star-{}", Uuid::new_v4().simple()));
    let bogus_ref = EntityRef::GatewayRoute(bogus.clone());
    let bogus_entity = repo
        .get_entity(EntityKind::GatewayRoute, bogus.as_str())
        .await
        .expect("get_entity bogus");
    let bogus_rules = repo
        .list_rules_for_entity(EntityKind::GatewayRoute, bogus.as_str())
        .await
        .expect("list bogus rules");
    let bogus_decision = resolve(ResolveInput {
        entity: &bogus_ref,
        rules: &bogus_rules,
        user_id: &admin,
        user_roles: &["admin".to_owned()],
        default_included: bogus_entity.map(|e| e.default_included),
        parents: &[],
    });
    assert!(
        matches!(
            bogus_decision,
            Decision::Deny {
                reason: DenyReason::UnknownEntity { .. }
            }
        ),
        "an unreconciled route id must deny as UnknownEntity, got {bogus_decision:?}"
    );

    cleanup(&f.pg, &f.default_route_id).await;
}
