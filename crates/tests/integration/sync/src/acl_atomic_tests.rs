//! Atomicity of `AccessControlIngestionService::ingest_config` when
//! `delete_orphans=true`: the DELETE of stale role/department rules and
//! the INSERT of the new set must happen inside one transaction so a
//! concurrent reader cannot observe an empty / partially-rebuilt rule
//! set (which would briefly read as "no rules → fall back to default
//! which may be allow").

use std::sync::Arc;
use std::time::Duration;

use systemprompt_security::authz::types::EntityKind;
use systemprompt_security::authz::{
    AccessControlConfig, AccessControlIngestionService, AccessControlRepository, IngestOptions,
    RuleEntry,
};

use crate::support::{try_db, wipe_rules};

fn cfg(entity_id: &str, rules: Vec<RuleEntry>) -> AccessControlConfig {
    let mut depts = std::collections::HashSet::new();
    for r in &rules {
        for d in &r.departments {
            depts.insert(d.clone());
        }
    }
    AccessControlConfig {
        departments: depts
            .into_iter()
            .map(|name| systemprompt_security::authz::DepartmentEntry {
                name,
                description: None,
                manager_email: None,
            })
            .collect(),
        rules: rules
            .into_iter()
            .map(|mut r| {
                if r.entity_id.is_empty() {
                    r.entity_id = entity_id.to_owned();
                }
                r
            })
            .collect(),
    }
}

fn role_rule(entity_id: &str, role: &str, allow: bool) -> RuleEntry {
    RuleEntry {
        entity_type: EntityKind::McpServer,
        entity_id: entity_id.to_owned(),
        access: if allow {
            systemprompt_security::authz::types::Access::Allow
        } else {
            systemprompt_security::authz::types::Access::Deny
        },
        roles: vec![role.to_owned()],
        departments: Vec::new(),
        justification: None,
    }
}

#[tokio::test]
async fn rename_replaces_old_rule_with_new_atomically() {
    let Some(db) = try_db().await else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let entity_id = "sync-it-acl-rename";
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;

    let svc = AccessControlIngestionService::new(&db).expect("svc");

    let initial = cfg(
        entity_id,
        vec![role_rule(entity_id, "engineer-old", true)],
    );
    svc.ingest_config(
        &initial,
        IngestOptions {
            override_existing: true,
            delete_orphans: true,
        },
    )
    .await
    .expect("seed");

    let renamed = cfg(
        entity_id,
        vec![role_rule(entity_id, "engineer-new", true)],
    );
    svc.ingest_config(
        &renamed,
        IngestOptions {
            override_existing: true,
            delete_orphans: true,
        },
    )
    .await
    .expect("rename");

    let repo = AccessControlRepository::new(&db).expect("repo");
    let rules = repo
        .list_rules_for_entity(EntityKind::McpServer, entity_id)
        .await
        .expect("list");
    let roles: Vec<&str> = rules.iter().map(|r| r.rule_value.as_str()).collect();
    assert!(
        !roles.contains(&"engineer-old"),
        "rename must drop the old role rule, got {roles:?}"
    );
    assert!(
        roles.contains(&"engineer-new"),
        "rename must install the new role rule, got {roles:?}"
    );

    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;
}

#[tokio::test]
async fn concurrent_reader_during_replace_never_sees_empty_state() {
    let Some(db) = try_db().await else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let entity_id = "sync-it-acl-concurrent";
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;

    let svc = AccessControlIngestionService::new(&db).expect("svc");
    let repo = Arc::new(AccessControlRepository::new(&db).expect("repo"));

    let seed = cfg(
        entity_id,
        vec![
            role_rule(entity_id, "alpha", true),
            role_rule(entity_id, "beta", true),
        ],
    );
    svc.ingest_config(
        &seed,
        IngestOptions {
            override_existing: true,
            delete_orphans: true,
        },
    )
    .await
    .expect("seed");

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_clone = stop.clone();
    let repo_clone = repo.clone();
    let entity_id_owned = entity_id.to_owned();
    let reader = tokio::spawn(async move {
        let mut empties = 0usize;
        let mut samples = 0usize;
        while !stop_clone.load(std::sync::atomic::Ordering::SeqCst) {
            let rules = repo_clone
                .list_rules_for_entity(EntityKind::McpServer, &entity_id_owned)
                .await
                .expect("list");
            samples += 1;
            if rules.is_empty() {
                empties += 1;
            }
            tokio::time::sleep(Duration::from_micros(50)).await;
        }
        (samples, empties)
    });

    let replacement = cfg(
        entity_id,
        vec![
            role_rule(entity_id, "gamma", true),
            role_rule(entity_id, "delta", true),
        ],
    );
    for _ in 0..20 {
        svc.ingest_config(
            &replacement,
            IngestOptions {
                override_existing: true,
                delete_orphans: true,
            },
        )
        .await
        .expect("replace");
        svc.ingest_config(
            &seed,
            IngestOptions {
                override_existing: true,
                delete_orphans: true,
            },
        )
        .await
        .expect("re-seed");
    }
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let (samples, empties) = reader.await.expect("reader");

    assert!(samples > 0, "reader sampled at least once");
    assert_eq!(
        empties, 0,
        "concurrent reader observed {empties}/{samples} empty rule sets — DELETE+INSERT is leaking out of the transaction",
    );

    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;
}

#[tokio::test]
async fn delete_orphans_preserves_user_overrides_and_sentinels() {
    let Some(db) = try_db().await else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let entity_id = "sync-it-acl-preserve";
    let pool = db.write_pool_arc().expect("pool");
    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;

    // Seed a user-override and a __default__ sentinel directly — these
    // must survive `delete_orphans=true`.
    let user_rule_id = systemprompt_identifiers::RuleId::generate();
    let sentinel_id = systemprompt_identifiers::RuleId::generate();
    sqlx::query!(
        r#"INSERT INTO access_control_rules
            (id, entity_type, entity_id, rule_type, rule_value, access, default_included, justification)
            VALUES ($1, 'mcp_server', $2, 'user', 'user-x', 'allow', false, NULL),
                   ($3, 'mcp_server', $2, 'role', '__default__', 'allow', true, NULL)"#,
        user_rule_id.as_str(),
        entity_id,
        sentinel_id.as_str(),
    )
    .execute(&*pool)
    .await
    .expect("seed direct rows");

    let svc = AccessControlIngestionService::new(&db).expect("svc");
    let new_cfg = cfg(entity_id, vec![role_rule(entity_id, "engineer", true)]);
    svc.ingest_config(
        &new_cfg,
        IngestOptions {
            override_existing: true,
            delete_orphans: true,
        },
    )
    .await
    .expect("ingest");

    let survivors: Vec<(String, String)> = sqlx::query!(
        r#"SELECT rule_type, rule_value FROM access_control_rules
           WHERE entity_type = 'mcp_server' AND entity_id = $1
           ORDER BY rule_type, rule_value"#,
        entity_id
    )
    .fetch_all(&*pool)
    .await
    .expect("list")
    .into_iter()
    .map(|r| (r.rule_type, r.rule_value))
    .collect();

    assert!(
        survivors.iter().any(|(t, v)| t == "user" && v == "user-x"),
        "user override must be preserved across YAML re-ingest, got {survivors:?}",
    );
    assert!(
        survivors
            .iter()
            .any(|(t, v)| t == "role" && v == "__default__"),
        "__default__ sentinel must survive delete_orphans, got {survivors:?}",
    );
    assert!(
        survivors
            .iter()
            .any(|(t, v)| t == "role" && v == "engineer"),
        "new YAML role must be present, got {survivors:?}",
    );

    wipe_rules(&db, EntityKind::McpServer.as_str(), entity_id).await;
}
