//! Integration-level deny-overrides semantics. YAML drives the rules
//! into the DB; the repository hands them to the resolver; the resolver
//! must deny when a deny rule applies — regardless of YAML ordering.

use systemprompt_identifiers::UserId;
use systemprompt_security::authz::types::{Access, Decision, EntityKind};
use systemprompt_security::authz::{
    AccessControlConfig, AccessControlIngestionService, AccessControlRepository,
    DepartmentEntry, IngestOptions, RuleEntry, resolve,
};

use crate::support::{try_db, wipe_rules};

fn yaml_with(rules: Vec<RuleEntry>, depts: Vec<&str>) -> AccessControlConfig {
    AccessControlConfig {
        departments: depts
            .into_iter()
            .map(|n| DepartmentEntry {
                name: n.to_owned(),
                description: None,
                manager_email: None,
            })
            .collect(),
        rules,
    }
}

#[tokio::test]
async fn role_deny_overrides_role_allow_for_same_subject() {
    let Some(db) = try_db().await else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let entity_id = "sync-it-deny-vs-allow";
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;

    let cfg = yaml_with(
        vec![
            RuleEntry {
                entity_type: EntityKind::McpServer,
                entity_id: entity_id.to_owned(),
                access: Access::Allow,
                roles: vec!["engineer".to_owned()],
                departments: vec![],
                justification: None,
            },
            RuleEntry {
                entity_type: EntityKind::McpServer,
                entity_id: entity_id.to_owned(),
                access: Access::Deny,
                roles: vec!["engineer".to_owned()],
                departments: vec![],
                justification: Some("emergency lockout".to_owned()),
            },
        ],
        vec![],
    );

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

    let decision = resolve(
        &rules,
        &UserId::new("user-1"),
        &["engineer".to_owned()],
        "",
        false,
    );
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

    let cfg = yaml_with(
        vec![RuleEntry {
            entity_type: EntityKind::McpServer,
            entity_id: entity_id.to_owned(),
            access: Access::Allow,
            roles: vec!["admin".to_owned()],
            departments: vec![],
            justification: None,
        }],
        vec![],
    );
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
           (id, entity_type, entity_id, rule_type, rule_value, access, default_included, justification)
           VALUES ($1, 'mcp_server', $2, 'user', 'banned-user', 'deny', false, NULL)"#,
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

    let decision = resolve(
        &rules,
        &UserId::new("banned-user"),
        &["admin".to_owned()],
        "",
        false,
    );
    assert!(
        matches!(decision, Decision::Deny { .. }),
        "user-level deny must override role-level allow, got {decision:?}",
    );

    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;
}

#[tokio::test]
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
            entity_id: id.to_owned(),
            access: Access::Allow,
            roles: vec!["dev".to_owned()],
            departments: vec![],
            justification: None,
        };
        let deny = RuleEntry {
            entity_type: EntityKind::McpServer,
            entity_id: id.to_owned(),
            access: Access::Deny,
            roles: vec!["dev".to_owned()],
            departments: vec![],
            justification: None,
        };
        yaml_with(
            if allow_first {
                vec![allow, deny]
            } else {
                vec![deny, allow]
            },
            vec![],
        )
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
    let decision_a = resolve(&rules_a, &user, &roles, "", false);
    let decision_b = resolve(&rules_b, &user, &roles, "", false);
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
