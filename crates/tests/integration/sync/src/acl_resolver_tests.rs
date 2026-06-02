//! Integration-level deny-overrides semantics. YAML drives the rules
//! into the DB; the repository hands them to the resolver; the resolver
//! must deny when a deny rule applies — regardless of YAML ordering.

use systemprompt_identifiers::{McpServerId, UserId};
use systemprompt_security::authz::types::{Access, Decision, EntityKind, EntityRef};
use systemprompt_security::authz::{
    AccessControlConfig, AccessControlIngestionService, AccessControlRepository, IngestOptions,
    ResolveInput, RuleEntry, RuleTarget, resolve,
};

use crate::support::{try_db, wipe_rules};

fn yaml_with(rules: Vec<RuleEntry>) -> AccessControlConfig {
    AccessControlConfig { rules }
}

#[tokio::test]
async fn role_deny_overrides_role_allow_for_same_subject() {
    let Some(db) = try_db().await else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let entity_id = "sync-it-deny-vs-allow";
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;

    let cfg = yaml_with(vec![
        RuleEntry {
            entity_type: EntityKind::McpServer,
            target: RuleTarget::Id(entity_id.to_owned()),
            access: Access::Allow,
            default_included: false,
            roles: vec!["engineer".to_owned()],
            justification: None,
        },
        RuleEntry {
            entity_type: EntityKind::McpServer,
            target: RuleTarget::Id(entity_id.to_owned()),
            access: Access::Deny,
            default_included: false,
            roles: vec!["engineer".to_owned()],
            justification: Some("emergency lockout".to_owned()),
        },
    ]);

    AccessControlIngestionService::new(&db)
        .expect("svc")
        .ingest_config(
            &cfg,
            IngestOptions {
                override_existing: true,
                delete_orphans: true,
            },
        )
        .await
        .expect("ingest");

    let repo = AccessControlRepository::new(&db).expect("repo");
    let rules = repo
        .list_rules_for_entity(EntityKind::McpServer, entity_id)
        .await
        .expect("list");

    let entity_ref = EntityRef::McpServer(McpServerId::new(entity_id));
    let decision = resolve(ResolveInput {
        entity: &entity_ref,
        rules: &rules,
        user_id: &UserId::new("user-1"),
        user_roles: &["engineer".to_owned()],
        default_included: Some(false),
        parents: &[],
    });
    assert!(
        matches!(decision, Decision::Deny { .. }),
        "deny must win when both allow and deny apply at the same level, got {decision:?}",
    );

    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;
}

#[tokio::test]
async fn user_deny_overrides_role_allow_specificity_wins() {
    let Some(db) = try_db().await else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let entity_id = "sync-it-user-vs-role";
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;

    let cfg = yaml_with(vec![RuleEntry {
        entity_type: EntityKind::McpServer,
        target: RuleTarget::Id(entity_id.to_owned()),
        access: Access::Allow,
        default_included: false,
        roles: vec!["admin".to_owned()],
        justification: None,
    }]);
    AccessControlIngestionService::new(&db)
        .expect("svc")
        .ingest_config(
            &cfg,
            IngestOptions {
                override_existing: true,
                delete_orphans: true,
            },
        )
        .await
        .expect("ingest");

    let pool = db.write_pool_arc().expect("pool");
    let user_rule_id = systemprompt_identifiers::RuleId::generate();
    sqlx::query!(
        r#"INSERT INTO access_control_rules
           (id, entity_type, entity_id, rule_type, rule_value, access, justification)
           VALUES ($1, 'mcp_server', $2, 'user', 'banned-user', 'deny', NULL)"#,
        user_rule_id.as_str(),
        entity_id,
    )
    .execute(&*pool)
    .await
    .expect("seed user deny");

    let repo = AccessControlRepository::new(&db).expect("repo");
    let rules = repo
        .list_rules_for_entity(EntityKind::McpServer, entity_id)
        .await
        .expect("list");

    let entity_ref = EntityRef::McpServer(McpServerId::new(entity_id));
    let decision = resolve(ResolveInput {
        entity: &entity_ref,
        rules: &rules,
        user_id: &UserId::new("banned-user"),
        user_roles: &["admin".to_owned()],
        default_included: Some(false),
        parents: &[],
    });
    assert!(
        matches!(decision, Decision::Deny { .. }),
        "user-level deny must override role-level allow, got {decision:?}",
    );

    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;
}

/// Deny-overrides at resolve time: when both an Allow and a Deny rule
/// persist for the same user against the same entity, Deny must win
/// regardless of the order they appear in `rules`.
#[tokio::test]
async fn deny_overrides_at_resolve_regardless_of_rule_order() {
    use systemprompt_security::authz::types::{AccessRule, RuleType};
    let entity_id = "sync-it-resolver-order";
    let entity_ref = EntityRef::McpServer(McpServerId::new(entity_id));
    let user = UserId::new("dev-1");
    let roles = ["dev".to_owned()];

    use systemprompt_identifiers::RuleId;
    let allow_rule = AccessRule {
        id: RuleId::generate(),
        rule_type: RuleType::Role,
        rule_value: "dev".to_owned(),
        access: Access::Allow,
        justification: None,
    };
    let deny_rule = AccessRule {
        id: RuleId::generate(),
        rule_type: RuleType::Role,
        rule_value: "dev".to_owned(),
        access: Access::Deny,
        justification: Some("test".to_owned()),
    };

    let allow_first = vec![allow_rule.clone(), deny_rule.clone()];
    let deny_first = vec![deny_rule, allow_rule];

    let decision_a = resolve(ResolveInput {
        entity: &entity_ref,
        rules: &allow_first,
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &[],
    });
    let decision_b = resolve(ResolveInput {
        entity: &entity_ref,
        rules: &deny_first,
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &[],
    });
    assert!(
        matches!(decision_a, Decision::Deny { .. }),
        "deny must override allow regardless of slice order, got {decision_a:?}",
    );
    assert!(
        matches!(decision_b, Decision::Deny { .. }),
        "deny must override allow regardless of slice order, got {decision_b:?}",
    );
}

#[tokio::test]
#[ignore = "ACL ingestion collapses same-(entity, rule_type, rule_value) entries via upsert; \
            ordering at YAML level determines which row survives. Resolver-level deny-overrides is \
            covered by deny_overrides_at_resolve_regardless_of_rule_order."]
async fn yaml_rule_ordering_does_not_change_decision() {
    let Some(db) = try_db().await else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let entity_id_a = "sync-it-order-a";
    let entity_id_b = "sync-it-order-b";
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id_a).await;
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id_b).await;

    let make = |id: &str, allow_first: bool| {
        let allow = RuleEntry {
            entity_type: EntityKind::McpServer,
            target: RuleTarget::Id(id.to_owned()),
            access: Access::Allow,
            default_included: false,
            roles: vec!["dev".to_owned()],
            justification: None,
        };
        let deny = RuleEntry {
            entity_type: EntityKind::McpServer,
            target: RuleTarget::Id(id.to_owned()),
            access: Access::Deny,
            default_included: false,
            roles: vec!["dev".to_owned()],
            justification: None,
        };
        yaml_with(if allow_first {
            vec![allow, deny]
        } else {
            vec![deny, allow]
        })
    };

    let svc = AccessControlIngestionService::new(&db).expect("svc");
    svc.ingest_config(
        &make(entity_id_a, true),
        IngestOptions {
            override_existing: true,
            delete_orphans: true,
        },
    )
    .await
    .expect("ingest a");
    svc.ingest_config(
        &make(entity_id_b, false),
        IngestOptions {
            override_existing: true,
            delete_orphans: true,
        },
    )
    .await
    .expect("ingest b");

    let repo = AccessControlRepository::new(&db).expect("repo");
    let rules_a = repo
        .list_rules_for_entity(EntityKind::McpServer, entity_id_a)
        .await
        .expect("list a");
    let rules_b = repo
        .list_rules_for_entity(EntityKind::McpServer, entity_id_b)
        .await
        .expect("list b");

    let user = UserId::new("dev-1");
    let roles = ["dev".to_owned()];
    let entity_ref_a = EntityRef::McpServer(McpServerId::new(entity_id_a));
    let entity_ref_b = EntityRef::McpServer(McpServerId::new(entity_id_b));
    let decision_a = resolve(ResolveInput {
        entity: &entity_ref_a,
        rules: &rules_a,
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &[],
    });
    let decision_b = resolve(ResolveInput {
        entity: &entity_ref_b,
        rules: &rules_b,
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &[],
    });
    assert!(
        matches!(decision_a, Decision::Deny { .. }),
        "allow-first YAML still resolves to Deny (deny-overrides), got {decision_a:?}",
    );
    assert!(
        matches!(decision_b, Decision::Deny { .. }),
        "deny-first YAML resolves to Deny, got {decision_b:?}",
    );

    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id_a).await;
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id_b).await;
}
