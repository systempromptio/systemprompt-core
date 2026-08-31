//! DB-backed coverage for gateway-route entity reconciliation and the wildcard
//! grant that authorizes the *synthesized* catch-all route.
//!
//! This is the end-to-end proof that the unit invariants
//! (`dispatchable_route_ids_*`) cannot give: that
//! `reconcile_gateway_entities_exact` actually materializes a content-addressed
//! `star-*` id into `access_control_entities`, that a `entity_match: "*"` rule
//! expands onto that code-synthesized id (closing the implicit YAML-vs-code
//! coupling), and that the resolver then allows a granted role while still
//! denying an id that has no catalog row (`UnknownEntity`, fail-closed).
//!
//! Each test scopes itself to a unique provider/default-provider so runs
//! against the shared `DATABASE_URL` never collide, and cleans up its rows.
//! The exact reconcile prunes every `gateway_route` row outside its set, so
//! two of these interleaved delete each other's rows. They are serialised by
//! the `gateway-reconcile-db` nextest group, not in-process: nextest runs each
//! test in its own process, so a `static Mutex` here would never contend and
//! would only look like protection.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName, UserId};
use systemprompt_models::profile::{
    ApiSurface, GatewayConfig, ProviderEntry, ProviderModel, ProviderRegistry, WireProtocol,
    synthesize_route_id,
};
use systemprompt_security::authz::resolver::{ResolveInput, resolve};
use systemprompt_security::authz::{
    Access, AccessControlConfig, AccessControlIngestionService, AccessControlRepository, Decision,
    DenyReason, EntityKind, EntityRef, IngestOptions, RegisteredEntities, RuleEntry, RuleTarget,
    reconcile_gateway_entities_exact,
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
            governance: Default::default(),
            extra_headers: HashMap::new(),
            models: vec![ProviderModel {
                id: ModelId::new("any"),
                aliases: Vec::new(),
                governance: None,
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
    reconcile_gateway_entities_exact(&repo, &id_refs, "test:gateway_reconcile")
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
            &RegisteredEntities::default(),
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
        attributes: &systemprompt_security::authz::NO_SUBJECT_ATTRIBUTES,
        dimensions: &[],
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
        attributes: &systemprompt_security::authz::NO_SUBJECT_ATTRIBUTES,
        dimensions: &[],
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

fn literal_gateway_rule(id: &str, roles: &[&str]) -> AccessControlConfig {
    AccessControlConfig {
        rules: vec![RuleEntry {
            entity_type: EntityKind::GatewayRoute,
            target: RuleTarget::Id(id.to_owned()),
            access: Access::Allow,
            default_included: true,
            roles: roles.iter().map(|r| (*r).to_owned()).collect(),
            justification: None,
        }],
    }
}

async fn entity_exists(pg: &PgPool, id: &str) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM access_control_entities WHERE entity_type='gateway_route' AND \
         entity_id=$1",
    )
    .bind(id)
    .fetch_one(pg)
    .await
    .expect("count entities")
        > 0
}

// The regression this whole mechanism exists for: a hand-written route id used
// to be accepted, mint its own catalog row, and leave a grant on a route that
// can never dispatch. Four such ids sat in roles.yaml across three repos for
// months because every boot made them look real.
#[tokio::test]
async fn ingest_rejects_a_literal_route_id_the_registry_does_not_vouch_for_and_writes_nothing() {
    let f = setup().await;
    let repo = AccessControlRepository::new(&f.db).expect("repo");

    let gateway = gateway_with_default(&f.provider);
    let registry_ids = gateway.dispatchable_route_ids(&registry(&f.provider));
    let real_id = registry_ids.first().expect("one route").clone();
    let id_refs: Vec<&str> = registry_ids.iter().map(RouteId::as_str).collect();
    reconcile_gateway_entities_exact(&repo, &id_refs, "profile:test")
        .await
        .expect("reconcile");

    let phantom = format!("claude-opus-4-8-gemini-{}", f.provider);
    let registered =
        RegisteredEntities::new().with_kind(EntityKind::GatewayRoute, id_refs.iter().copied());

    let svc = AccessControlIngestionService::from_pool(Arc::clone(&f.pg));
    let err = svc
        .ingest_config(
            &literal_gateway_rule(&phantom, &["user"]),
            IngestOptions {
                override_existing: true,
                delete_orphans: false,
            },
            &registered,
        )
        .await
        .expect_err("a route id no profile route provides must be rejected");

    let msg = err.to_string();
    assert!(msg.contains(&phantom), "error must name the id: {msg}");
    assert!(
        msg.contains("entity_match"),
        "error must say what to do instead: {msg}"
    );

    // The point of failing rather than minting: nothing persisted, so the id
    // does not look real on the next run.
    assert!(
        !entity_exists(&f.pg, &phantom).await,
        "rejected id must not leave a catalog row behind"
    );
    assert!(
        role_values(&f.pg, &RouteId::new(phantom.clone()))
            .await
            .is_empty(),
        "rejected id must not leave a grant behind"
    );

    // A real route in the same shape still ingests, so the check rejects the id
    // and not the rule form.
    svc.ingest_config(
        &literal_gateway_rule(real_id.as_str(), &["user"]),
        IngestOptions {
            override_existing: true,
            delete_orphans: false,
        },
        &registered,
    )
    .await
    .expect("a registered route id must still be accepted");
    assert_eq!(role_values(&f.pg, &real_id).await, vec!["user".to_owned()]);

    cleanup(&f.pg, &real_id).await;
}

// Unenforced kinds keep the old behaviour, so adopting this is opt-in per
// kind.
#[tokio::test]
async fn ingest_still_self_materializes_when_the_kind_is_not_enforced() {
    let f = setup().await;
    let phantom = RouteId::new(format!("unenforced-{}", f.provider));

    let svc = AccessControlIngestionService::from_pool(Arc::clone(&f.pg));
    svc.ingest_config(
        &literal_gateway_rule(phantom.as_str(), &["user"]),
        IngestOptions {
            override_existing: true,
            delete_orphans: false,
        },
        &RegisteredEntities::default(),
    )
    .await
    .expect("an unenforced kind must keep self-materialising");

    assert!(entity_exists(&f.pg, phantom.as_str()).await);
    cleanup(&f.pg, &phantom).await;
}

// `reconcile_gateway_entities_exact` must remove catalog rows no route claims,
// and must refuse to do so from an empty set (which would empty the catalog).
#[tokio::test]
async fn exact_reconcile_prunes_stale_rows_and_refuses_an_empty_route_set() {
    let f = setup().await;
    let repo = AccessControlRepository::new(&f.db).expect("repo");

    let gateway = gateway_with_default(&f.provider);
    let ids = gateway.dispatchable_route_ids(&registry(&f.provider));
    let id_refs: Vec<&str> = ids.iter().map(RouteId::as_str).collect();

    let stale = RouteId::new(format!("stale-{}", f.provider));
    repo.ensure_entity(EntityKind::GatewayRoute, stale.as_str(), "profile:old")
        .await
        .expect("seed a stale row");
    assert!(entity_exists(&f.pg, stale.as_str()).await);

    let report = reconcile_gateway_entities_exact(&repo, &id_refs, "profile:test")
        .await
        .expect("exact reconcile");
    assert_eq!(report.registered, id_refs.len());
    assert!(report.pruned >= 1, "the stale row should have been pruned");
    assert!(
        !entity_exists(&f.pg, stale.as_str()).await,
        "a row no route claims must not survive"
    );
    assert!(
        entity_exists(&f.pg, id_refs[0]).await,
        "a live route must survive"
    );

    reconcile_gateway_entities_exact(&repo, &[], "profile:test")
        .await
        .expect_err("an empty route set must be refused, not obeyed");
    assert!(
        entity_exists(&f.pg, id_refs[0]).await,
        "the refused call must not have deleted anything"
    );

    cleanup(&f.pg, &ids[0]).await;
}

// `ensure_entity` satisfies the FK without widening access: an absent row is
// created closed, and a present row keeps whatever `default_included` it had.
#[tokio::test]
async fn ensure_entity_creates_closed_and_never_overwrites_default_included() {
    let f = setup().await;
    let repo = AccessControlRepository::new(&f.db).expect("repo");
    let id = RouteId::new(format!("ensure-{}", f.provider));

    repo.ensure_entity(EntityKind::GatewayRoute, id.as_str(), "test:ensure")
        .await
        .expect("ensure absent");
    let created = repo
        .get_entity(EntityKind::GatewayRoute, id.as_str())
        .await
        .expect("get")
        .expect("row created");
    assert!(
        !created.default_included,
        "an ensured row must start closed"
    );

    repo.upsert_entity(EntityKind::GatewayRoute, id.as_str(), true, "test:open")
        .await
        .expect("open it");
    repo.ensure_entity(EntityKind::GatewayRoute, id.as_str(), "test:ensure")
        .await
        .expect("ensure present");
    let kept = repo
        .get_entity(EntityKind::GatewayRoute, id.as_str())
        .await
        .expect("get")
        .expect("row still present");
    assert!(
        kept.default_included,
        "ensure_entity must leave an existing default_included alone"
    );
    assert_eq!(
        kept.source, "test:open",
        "ensure_entity must not relabel a present row"
    );

    cleanup(&f.pg, &id).await;
}
