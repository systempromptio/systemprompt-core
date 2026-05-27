//! Transactional rollback when a YAML bundle fails validation. The
//! pre-existing rule set must remain intact — no half-written ACL state.

use systemprompt_security::authz::types::{Access, EntityKind};
use systemprompt_security::authz::{
    AccessControlConfig, AccessControlIngestionService, AccessControlRepository, IngestOptions,
    RuleEntry,
};

use crate::support::{try_db, wipe_rules};

#[tokio::test]
async fn invalid_yaml_leaves_existing_rules_untouched() {
    let Some(db) = try_db().await else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let entity_id = "sync-it-acl-rollback";
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;

    let svc = AccessControlIngestionService::new(&db).expect("svc");
    let repo = AccessControlRepository::new(&db).expect("repo");

    let seed = AccessControlConfig {
        rules: vec![RuleEntry {
            entity_type: EntityKind::McpServer,
            entity_id: entity_id.to_owned(),
            access: Access::Allow,
            roles: vec!["seeded-role".to_owned()],
            justification: None,
        }],
    };
    svc.ingest_config(
        &seed,
        IngestOptions {
            override_existing: true,
            delete_orphans: true,
        },
    )
    .await
    .expect("seed");

    let bad = AccessControlConfig {
        rules: vec![
            RuleEntry {
                entity_type: EntityKind::McpServer,
                entity_id: entity_id.to_owned(),
                access: Access::Allow,
                roles: vec!["new-role".to_owned()],
                justification: None,
            },
            RuleEntry {
                entity_type: EntityKind::McpServer,
                entity_id: entity_id.to_owned(),
                access: Access::Allow,
                roles: vec![],
                justification: None,
            },
        ],
    };
    let res = svc
        .ingest_config(
            &bad,
            IngestOptions {
                override_existing: true,
                delete_orphans: true,
            },
        )
        .await;
    assert!(
        res.is_err(),
        "validation failure must surface as an error, got {res:?}",
    );

    let rules = repo
        .list_rules_for_entity(EntityKind::McpServer, entity_id)
        .await
        .expect("list");
    let roles: Vec<&str> = rules.iter().map(|r| r.rule_value.as_str()).collect();
    assert!(
        roles.contains(&"seeded-role"),
        "validation failure rolled back: seed role must survive, got {roles:?}",
    );
    assert!(
        !roles.contains(&"new-role"),
        "no rule from the rejected bundle should leak into the DB, got {roles:?}",
    );

    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;
}
