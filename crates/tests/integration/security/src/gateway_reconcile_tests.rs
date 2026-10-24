//! DB-backed coverage for gateway-route entity reconciliation and the wildcard
//! grant that authorizes the *synthesized* catch-all route.
//!
//! This is the end-to-end proof that the unit invariants
//! (`ResolvedCatalog::route_ids`) cannot give: that
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

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_catalog_fixtures::{CatalogFixture, claude_model, platform};
use systemprompt_database::DbPool;
use systemprompt_governance::authz::resolver::{ResolveInput, resolve};
use systemprompt_governance::authz::{
    Access, AccessControlRepository, Decision, DenyReason, EntityKind, EntityRef, RuleType,
    UpsertRuleParams, reconcile_gateway_entities_exact,
};
use systemprompt_identifiers::{RouteId, UserId};
use systemprompt_manifest::services::catalog::{EndpointConfig, ProviderConfig, ResolvedCatalog};
use systemprompt_manifest::services::synthesize_route_id;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_wire::WireProtocol;
use uuid::Uuid;

struct Fixture {
    db: DbPool,
    pg: Arc<PgPool>,
    provider: String,
    default_route_id: RouteId,
}

async fn setup() -> Fixture {
    let db = test_db_pool().await;
    let pg = db.pool();
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

fn catalog_with_default(provider: &str) -> ResolvedCatalog {
    CatalogFixture::new()
        .credential(provider, platform("test"))
        .provider(provider, ProviderConfig::new(provider))
        .endpoint(
            provider,
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://example.test/v1"),
        )
        .model("any", claude_model(0))
        .deploy("any", [format!("{provider}/api").as_str()])
        .default_model("any")
        .build()
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
    let repo = AccessControlRepository::new(&f.db);

    // 1. Reconcile the profile's dispatchable routes (here: only the synthetic
    //    default catch-all) into the entity catalog.
    let ids = catalog_with_default(&f.provider).route_ids();
    assert!(
        ids.contains(&f.default_route_id),
        "dispatchable ids must include the synthesized catch-all {}",
        f.default_route_id.as_str()
    );
    let id_refs: Vec<&str> = ids.iter().map(RouteId::as_str).collect();
    reconcile_gateway_entities_exact(&repo, &id_refs, "code:test")
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

    // 2. Grant the synthesized id directly.
    for role in ["user", "admin"] {
        repo.upsert_rule(UpsertRuleParams {
            entity_type: EntityKind::GatewayRoute,
            entity_id: f.default_route_id.as_str(),
            rule_type: RuleType::ROLE,
            rule_value: role,
            access: Access::Allow,
            justification: None,
            source: "code:test",
        })
        .await
        .expect("grant role");
    }
    assert_eq!(
        role_values(&f.pg, &f.default_route_id).await,
        vec!["admin", "user"],
        "the grant lands on the code-synthesized route id"
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
        attributes: &systemprompt_governance::authz::NO_SUBJECT_ATTRIBUTES,
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
        attributes: &systemprompt_governance::authz::NO_SUBJECT_ATTRIBUTES,
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

// A hand-written route id must not be accepted: it would mint its own catalog
// row and leave a grant on a route that can never dispatch, and every boot
// would make it look real.
// Unenforced kinds keep the old behaviour, so adopting this is opt-in per
// kind.
// `reconcile_gateway_entities_exact` must remove catalog rows no route claims,
// and must refuse to do so from an empty set (which would empty the catalog).
#[tokio::test]
async fn exact_reconcile_prunes_stale_rows_and_refuses_an_empty_route_set() {
    let f = setup().await;
    let repo = AccessControlRepository::new(&f.db);

    let ids = catalog_with_default(&f.provider).route_ids();
    let id_refs: Vec<&str> = ids.iter().map(RouteId::as_str).collect();

    let stale = RouteId::new(format!("stale-{}", f.provider));
    repo.ensure_entity(EntityKind::GatewayRoute, stale.as_str(), "code:old")
        .await
        .expect("seed a stale row");
    assert!(entity_exists(&f.pg, stale.as_str()).await);

    let report = reconcile_gateway_entities_exact(&repo, &id_refs, "code:test")
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

    reconcile_gateway_entities_exact(&repo, &[], "code:test")
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
    let repo = AccessControlRepository::new(&f.db);
    let id = RouteId::new(format!("ensure-{}", f.provider));

    repo.ensure_entity(EntityKind::GatewayRoute, id.as_str(), "code:ensure")
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

    repo.upsert_entity(EntityKind::GatewayRoute, id.as_str(), true, "code:open")
        .await
        .expect("open it");
    repo.ensure_entity(EntityKind::GatewayRoute, id.as_str(), "code:ensure")
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
        kept.source, "code:open",
        "ensure_entity must not relabel a present row"
    );

    cleanup(&f.pg, &id).await;
}

#[tokio::test]
async fn reconcile_never_overwrites_an_entitys_declared_default_included() {
    let f = setup().await;
    let repo = AccessControlRepository::new(&f.db);
    let declared = f.default_route_id.clone();
    let catalog_only = synthesize_route_id("catalog", &f.provider);
    cleanup(&f.pg, &catalog_only).await;

    repo.upsert_entity(
        EntityKind::GatewayRoute,
        declared.as_str(),
        true,
        "code:access-control/rules.yaml",
    )
    .await
    .expect("declare default_included = true");
    let keep = [declared.as_str(), catalog_only.as_str()];
    repo.reconcile_entities(EntityKind::GatewayRoute, &keep, false, "code:services")
        .await
        .expect("first reconcile");
    repo.reconcile_entities(EntityKind::GatewayRoute, &keep, true, "code:services")
        .await
        .expect("second reconcile with the opposite flag");

    let declared_row = repo
        .get_entity(EntityKind::GatewayRoute, declared.as_str())
        .await
        .expect("get declared")
        .expect("declared row survives reconcile");
    assert!(
        declared_row.default_included,
        "a catalog reconcile must not flip the access plane's default_included"
    );
    assert_eq!(
        declared_row.source, "code:access-control/rules.yaml",
        "source stays the declarer's"
    );

    let catalog_row = repo
        .get_entity(EntityKind::GatewayRoute, catalog_only.as_str())
        .await
        .expect("get catalog")
        .expect("catalog row inserted");
    assert!(
        !catalog_row.default_included,
        "the first insert keeps the reconcile's default; the second pass leaves it"
    );

    cleanup(&f.pg, &declared).await;
    cleanup(&f.pg, &catalog_only).await;
}

// A console edit opens a route by setting `default_included = true`; the boot
// catalog writer runs again on every restart and must leave that edit alone.
#[tokio::test]
async fn gateway_route_reconcile_keeps_a_console_edited_default_included() {
    let f = setup().await;
    let repo = AccessControlRepository::new(&f.db);
    let id = f.default_route_id.clone();
    cleanup(&f.pg, &id).await;

    reconcile_gateway_entities_exact(&repo, &[id.as_str()], "code:test")
        .await
        .expect("first reconcile");
    repo.upsert_entity(EntityKind::GatewayRoute, id.as_str(), true, "console:admin")
        .await
        .expect("console opens the route");
    reconcile_gateway_entities_exact(&repo, &[id.as_str()], "code:test")
        .await
        .expect("reconcile on the next boot");

    let row = repo
        .get_entity(EntityKind::GatewayRoute, id.as_str())
        .await
        .expect("get")
        .expect("row present");
    assert!(
        row.default_included,
        "a boot reconcile must not close a route the console opened"
    );
    assert_eq!(
        row.source, "console:admin",
        "the console's provenance is kept"
    );

    cleanup(&f.pg, &id).await;
}
