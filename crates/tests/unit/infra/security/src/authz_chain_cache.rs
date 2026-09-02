//! DB-backed tests for `ChainIndexCache`: the recheck window serves from
//! memory, a fingerprint change forces a reload, and TTL expiry reloads even
//! when nothing changed.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::DbPool;
use systemprompt_security::authz::{
    Access, AccessControlRepository, ChainIndexCache, ChainSources, EntityKind, RuleType,
    UpsertRuleParams,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

const KIND: EntityKind = EntityKind::Plugin;
const LONG: Duration = Duration::from_secs(3600);

async fn repo() -> Option<(AccessControlRepository, DbPool)> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let repo = AccessControlRepository::new(&db).ok()?;
    Some((repo, db))
}

fn unique_entity() -> String {
    format!("chain-cache-{}", Uuid::new_v4().simple())
}

async fn cleanup(db: &DbPool, entity_id: &str) {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM access_control_entities WHERE entity_type = $1 AND entity_id = $2")
        .bind(KIND.as_str())
        .bind(entity_id)
        .execute(&*pg)
        .await
        .expect("cleanup entities");
}

fn sources() -> Arc<ChainSources> {
    Arc::new(ChainSources::default())
}

async fn upsert_rule(repo: &AccessControlRepository, entity_id: &str, value: &str) {
    repo.upsert_rule(UpsertRuleParams {
        entity_type: KIND,
        entity_id,
        rule_type: RuleType::ROLE,
        rule_value: value,
        access: Access::Allow,
        justification: None,
    })
    .await
    .expect("upsert rule");
}

#[tokio::test]
async fn within_the_recheck_window_the_same_index_is_served_without_a_reload() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let id = unique_entity();
    repo.upsert_entity(KIND, &id, false, "test")
        .await
        .expect("entity");
    let cache = ChainIndexCache::new(LONG, LONG);

    let first = cache.get(&repo, sources()).await.expect("first load");
    upsert_rule(&repo, &id, "admin").await;
    let second = cache.get(&repo, sources()).await.expect("second get");

    assert!(
        Arc::ptr_eq(&first, &second),
        "a get inside the recheck window must not touch the database, even after a rule change"
    );
    cleanup(&db, &id).await;
}

#[tokio::test]
async fn an_upserted_rule_moves_the_fingerprint_and_forces_a_reload() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let id = unique_entity();
    repo.upsert_entity(KIND, &id, false, "test")
        .await
        .expect("entity");
    let cache = ChainIndexCache::new(LONG, Duration::ZERO);

    let before = cache.get(&repo, sources()).await.expect("first load");
    upsert_rule(&repo, &id, "admin").await;
    let after = cache.get(&repo, sources()).await.expect("get after upsert");

    assert!(
        !Arc::ptr_eq(&before, &after),
        "the rule's updated_at bump and count change must be seen once the recheck window has \
         elapsed"
    );
    cleanup(&db, &id).await;
}

#[tokio::test]
async fn a_deleted_rule_moves_the_fingerprint_and_forces_a_reload() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let id = unique_entity();
    repo.upsert_entity(KIND, &id, false, "test")
        .await
        .expect("entity");
    let rule = repo
        .upsert_rule(UpsertRuleParams {
            entity_type: KIND,
            entity_id: &id,
            rule_type: RuleType::ROLE,
            rule_value: "auditor",
            access: Access::Allow,
            justification: None,
        })
        .await
        .expect("rule");
    let cache = ChainIndexCache::new(LONG, Duration::ZERO);

    let before = cache.get(&repo, sources()).await.expect("first load");
    assert!(repo.delete_rule(&rule.id).await.expect("delete"));
    let after = cache.get(&repo, sources()).await.expect("get after delete");

    assert!(
        !Arc::ptr_eq(&before, &after),
        "a delete leaves no updated_at trace, so the row count must carry the change"
    );
    cleanup(&db, &id).await;
}

#[tokio::test]
async fn ttl_expiry_reloads_even_when_the_fingerprint_is_unchanged() {
    let Some((repo, _db)) = repo().await else {
        return;
    };
    let cache = ChainIndexCache::new(Duration::ZERO, Duration::ZERO);

    let first = cache.get(&repo, sources()).await.expect("first load");
    let second = cache.get(&repo, sources()).await.expect("second load");

    assert!(
        !Arc::ptr_eq(&first, &second),
        "an expired TTL must rebuild the index regardless of the fingerprint"
    );
}
