//! DB-backed behavioural tests for [`AccessControlRepository`].
//!
//! These exercise the entity catalog (`access_control_entities`) and the
//! per-grant rule table (`access_control_rules`) against the shared fixture
//! database. Every test scopes itself to a unique `entity_id` so concurrent
//! runs against the shared `DATABASE_URL` never collide, and removes its rows
//! on the way out. Tests skip cleanly when no fixture database is reachable.

use std::str::FromStr;

use systemprompt_database::DbPool;
use systemprompt_identifiers::RuleId;
use systemprompt_security::authz::{
    Access, AccessControlRepository, EntityKind, RuleType, UpsertRuleParams,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

const KIND: EntityKind = EntityKind::Skill;

async fn repo() -> Option<(AccessControlRepository, DbPool)> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let repo = AccessControlRepository::new(&db).ok()?;
    Some((repo, db))
}

fn unique_entity() -> String {
    format!("authz-test-{}", Uuid::new_v4().simple())
}

async fn cleanup(db: &DbPool, entity_id: &str) {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM access_control_rules WHERE entity_type = $1 AND entity_id = $2")
        .bind(KIND.as_str())
        .bind(entity_id)
        .execute(&*pg)
        .await
        .expect("cleanup rules");
    sqlx::query("DELETE FROM access_control_entities WHERE entity_type = $1 AND entity_id = $2")
        .bind(KIND.as_str())
        .bind(entity_id)
        .execute(&*pg)
        .await
        .expect("cleanup entities");
}

#[tokio::test]
async fn upsert_entity_then_get_roundtrips() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let id = unique_entity();

    assert!(
        repo.get_entity(KIND, &id).await.expect("get").is_none(),
        "unknown entity has no catalog row"
    );

    repo.upsert_entity(KIND, &id, true, "test-source")
        .await
        .expect("upsert entity");

    let row = repo
        .get_entity(KIND, &id)
        .await
        .expect("get")
        .expect("entity now exists");
    assert_eq!(row.kind, KIND);
    assert_eq!(row.id, id);
    assert!(row.default_included);
    assert_eq!(row.source, "test-source");

    repo.upsert_entity(KIND, &id, false, "second-source")
        .await
        .expect("re-upsert entity");
    let row = repo.get_entity(KIND, &id).await.expect("get").expect("row");
    assert!(
        !row.default_included,
        "the most recent bootstrap pass overwrites default_included"
    );
    assert_eq!(row.source, "second-source");

    cleanup(&db, &id).await;
}

#[tokio::test]
async fn upsert_entities_batch_inserts_and_list_returns_them() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let a = unique_entity();
    let b = unique_entity();

    repo.upsert_entities(KIND, &[a.as_str(), b.as_str()], true, "batch")
        .await
        .expect("batch upsert");

    let listed = repo.list_entities(KIND).await.expect("list");
    assert!(listed.iter().any(|e| e.id == a && e.source == "batch"));
    assert!(listed.iter().any(|e| e.id == b));

    repo.upsert_entities(KIND, &[], true, "noop")
        .await
        .expect("empty batch is a no-op");

    cleanup(&db, &a).await;
    cleanup(&db, &b).await;
}

#[tokio::test]
async fn upsert_rule_requires_an_entity_then_persists_and_lists() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let id = unique_entity();

    let orphan = repo
        .upsert_rule(UpsertRuleParams {
            entity_type: KIND,
            entity_id: &id,
            rule_type: RuleType::Role,
            rule_value: "admin",
            access: Access::Allow,
            justification: Some("ops"),
        })
        .await;
    assert!(
        orphan.is_err(),
        "a rule without a catalog entity must hit the foreign-key constraint"
    );

    repo.upsert_entity(KIND, &id, false, "test")
        .await
        .expect("register entity");

    let rule = repo
        .upsert_rule(UpsertRuleParams {
            entity_type: KIND,
            entity_id: &id,
            rule_type: RuleType::Role,
            rule_value: "admin",
            access: Access::Allow,
            justification: Some("ops"),
        })
        .await
        .expect("upsert rule");
    assert_eq!(rule.rule_type, RuleType::Role);
    assert_eq!(rule.rule_value, "admin");
    assert_eq!(rule.access, Access::Allow);
    assert_eq!(rule.justification.as_deref(), Some("ops"));

    let listed = repo
        .list_rules_for_entity(KIND, &id)
        .await
        .expect("list rules");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id.as_str(), rule.id.as_str());

    cleanup(&db, &id).await;
}

#[tokio::test]
async fn upsert_rule_conflict_updates_access_in_place() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let id = unique_entity();
    repo.upsert_entity(KIND, &id, false, "test")
        .await
        .expect("entity");

    let first = repo
        .upsert_rule(UpsertRuleParams {
            entity_type: KIND,
            entity_id: &id,
            rule_type: RuleType::User,
            rule_value: "alice",
            access: Access::Allow,
            justification: None,
        })
        .await
        .expect("first");

    let second = repo
        .upsert_rule(UpsertRuleParams {
            entity_type: KIND,
            entity_id: &id,
            rule_type: RuleType::User,
            rule_value: "alice",
            access: Access::Deny,
            justification: Some("revoked"),
        })
        .await
        .expect("conflict upsert");

    assert_eq!(
        first.id.as_str(),
        second.id.as_str(),
        "conflict updates the existing row, not a new one"
    );
    assert_eq!(second.access, Access::Deny);
    assert_eq!(second.justification.as_deref(), Some("revoked"));

    let listed = repo.list_rules_for_entity(KIND, &id).await.expect("list");
    assert_eq!(listed.len(), 1, "still a single grant after the conflict");

    cleanup(&db, &id).await;
}

#[tokio::test]
async fn list_rules_bulk_groups_by_entity_and_seeds_empty_ids() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let with_rule = unique_entity();
    let without_rule = unique_entity();
    repo.upsert_entity(KIND, &with_rule, false, "test")
        .await
        .expect("entity");
    repo.upsert_rule(UpsertRuleParams {
        entity_type: KIND,
        entity_id: &with_rule,
        rule_type: RuleType::Role,
        rule_value: "viewer",
        access: Access::Allow,
        justification: None,
    })
    .await
    .expect("rule");

    let empty = repo.list_rules_bulk(KIND, &[]).await.expect("empty bulk");
    assert!(empty.is_empty());

    let bulk = repo
        .list_rules_bulk(KIND, &[with_rule.clone(), without_rule.clone()])
        .await
        .expect("bulk");
    assert_eq!(
        bulk.get(&with_rule).map(Vec::len),
        Some(1),
        "entity with a grant lists exactly one rule"
    );
    assert_eq!(
        bulk.get(&without_rule).map(Vec::len),
        Some(0),
        "queried entity with no grants is seeded with an empty vec"
    );

    cleanup(&db, &with_rule).await;
    cleanup(&db, &without_rule).await;
}

#[tokio::test]
async fn set_justification_and_delete_rule_report_affected_rows() {
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
            rule_type: RuleType::Role,
            rule_value: "editor",
            access: Access::Allow,
            justification: Some("initial"),
        })
        .await
        .expect("rule");

    assert!(
        repo.set_justification(&rule.id, Some("updated"))
            .await
            .expect("set justification"),
        "updating an existing rule affects one row"
    );
    let after = repo.list_rules_for_entity(KIND, &id).await.expect("list");
    assert_eq!(after[0].justification.as_deref(), Some("updated"));

    assert!(
        repo.set_justification(&rule.id, None)
            .await
            .expect("clear justification"),
        "clearing the note still touches the row"
    );

    let missing = RuleId::generate();
    assert!(
        !repo
            .set_justification(&missing, Some("x"))
            .await
            .expect("set on missing"),
        "no row affected for an unknown rule id"
    );

    assert!(
        repo.delete_rule(&rule.id).await.expect("delete"),
        "deleting an existing rule reports a row"
    );
    assert!(
        !repo.delete_rule(&rule.id).await.expect("delete again"),
        "deleting an already-gone rule reports no row"
    );
    assert!(
        repo.list_rules_for_entity(KIND, &id)
            .await
            .expect("list")
            .is_empty(),
        "rule table is empty after delete"
    );

    cleanup(&db, &id).await;
}

#[tokio::test]
async fn list_role_rules_for_export_includes_role_grants_only() {
    let Some((repo, db)) = repo().await else {
        return;
    };
    let id = unique_entity();
    repo.upsert_entity(KIND, &id, false, "test")
        .await
        .expect("entity");
    repo.upsert_rule(UpsertRuleParams {
        entity_type: KIND,
        entity_id: &id,
        rule_type: RuleType::Role,
        rule_value: "auditor",
        access: Access::Allow,
        justification: None,
    })
    .await
    .expect("role rule");
    repo.upsert_rule(UpsertRuleParams {
        entity_type: KIND,
        entity_id: &id,
        rule_type: RuleType::User,
        rule_value: "bob",
        access: Access::Allow,
        justification: None,
    })
    .await
    .expect("user rule");

    let exported = repo.list_role_rules_for_export().await.expect("export");
    let ours: Vec<_> = exported.iter().filter(|r| r.entity_id == id).collect();
    assert_eq!(ours.len(), 1, "export carries the role grant only");
    let row = ours[0];
    assert_eq!(RuleType::from_str(&row.rule_type).unwrap(), RuleType::Role);
    assert_eq!(row.rule_value, "auditor");
    assert_eq!(Access::from_str(&row.access).unwrap(), Access::Allow);

    cleanup(&db, &id).await;
}
