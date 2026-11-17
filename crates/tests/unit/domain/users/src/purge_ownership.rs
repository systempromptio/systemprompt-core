//! Each member declares the purge of its own tables: a composition that leaves
//! an owning member out still admits its purge plan, the full composition
//! purges the same tables as before with each table under its owner, and a
//! member naming another member's table is refused whatever its standing.

use systemprompt_extension::{
    AdmittedPurge, OrphanSweep, PurgeAdmissionError, PurgeClaim, PurgeDeclaration,
    SchemaDefinition, UserPurgeTable,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::{fixture_admitted_without, schema_admitted};

const OWNED: &[(&str, &[&str])] = &[
    ("events", &["event_outbox"]),
    ("users", &["user_rate_limit_buckets", "sso_login_events"]),
    ("logging", &["logs"]),
    ("authz", &["governance_decisions"]),
    ("oauth", &["oauth_jti_revocations"]),
    (
        "mcp",
        &[
            "mcp_artifacts",
            "mcp_tool_executions",
            "mcp_sessions",
            "mcp_external_sessions",
            "artifact_payloads",
        ],
    ),
    ("ai", &["ai_requests"]),
    ("agent", &["agent_tasks", "task_messages"]),
    ("files", &["files"]),
];

fn full_plan() -> AdmittedPurge {
    let admitted = schema_admitted().expect("the core members admit");
    AdmittedPurge::of(admitted.built()).expect("the full composition's purge plan admits")
}

fn owner_of(plan: &AdmittedPurge, table: &str) -> Vec<String> {
    plan.entries()
        .iter()
        .filter(|entry| entry.declaration.table() == table)
        .map(|entry| entry.owner.to_string())
        .collect()
}

#[test]
fn every_formerly_users_declared_table_is_declared_once_by_its_owner() {
    let plan = full_plan();
    for (owner, tables) in OWNED {
        for table in *tables {
            assert_eq!(
                owner_of(&plan, table),
                vec![(*owner).to_owned()],
                "{table} is purged by {owner} alone"
            );
        }
    }
}

#[test]
fn the_full_composition_declares_the_same_shapes_as_before() {
    let plan = full_plan();
    for (_, tables) in OWNED {
        for table in tables.iter().filter(|t| **t != "artifact_payloads") {
            let declaration = plan
                .entries()
                .iter()
                .find(|entry| entry.declaration.table() == *table)
                .map(|entry| entry.declaration.clone());
            assert_eq!(
                declaration,
                Some(PurgeDeclaration::from(UserPurgeTable {
                    table,
                    column: "user_id",
                    predicate: None,
                })),
                "{table} keeps its column and has no predicate"
            );
        }
    }
    let sweep = plan
        .entries()
        .iter()
        .find(|entry| entry.declaration.is_sweep())
        .map(|entry| entry.declaration.clone());
    assert_eq!(
        sweep,
        Some(PurgeDeclaration::from(OrphanSweep {
            table: "artifact_payloads",
            key: "sha256",
            referenced_by: "mcp_artifacts",
            via: "payload_sha256",
            user_column: "user_id",
        }))
    );
}

#[test]
fn within_a_member_children_keep_their_order_relative_to_parents() {
    let plan = full_plan();
    let position = |table: &str| {
        plan.entries()
            .iter()
            .position(|entry| entry.declaration.table() == table)
            .unwrap_or_else(|| panic!("{table} is declared"))
    };
    assert!(position("mcp_artifacts") < position("mcp_tool_executions"));
    assert!(position("agent_tasks") < position("task_messages"));
}

#[test]
fn a_composition_without_an_owning_member_admits_its_purge_plan() {
    assert!(!full_plan().entries().is_empty(), "control: the full plan admits");
    for (owner, tables) in OWNED.iter().filter(|(owner, _)| *owner != "users") {
        let admitted = fixture_admitted_without(&[ExtensionId::new(*owner)]);
        let plan = AdmittedPurge::of(admitted.built())
            .unwrap_or_else(|e| panic!("without {owner} the purge plan admits: {e}"));
        assert!(!plan.contains(&ExtensionId::new(*owner)));
        for table in *tables {
            assert!(
                owner_of(&plan, table).is_empty(),
                "without {owner}, {table} is not in the admitted plan"
            );
        }
    }
}

fn schemas(tables: &[&str]) -> Vec<SchemaDefinition> {
    tables
        .iter()
        .map(|t| SchemaDefinition::new(*t, format!("CREATE TABLE {t} (user_id TEXT, sha256 TEXT)")))
        .collect()
}

#[test]
fn a_core_member_naming_a_core_peers_table_is_refused() {
    let users = ExtensionId::new("users");
    let mcp = ExtensionId::new("mcp");
    let users_schemas = schemas(&["users"]);
    let mcp_schemas = schemas(&["mcp_artifacts"]);
    let foreign = [PurgeDeclaration::from(UserPurgeTable {
        table: "mcp_artifacts",
        column: "user_id",
        predicate: None,
    })];
    let claims = |users_decl: &[PurgeDeclaration], mcp_decl: &[PurgeDeclaration]| {
        AdmittedPurge::admit(&[
            PurgeClaim {
                owner: &users,
                schemas: &users_schemas,
                declarations: users_decl,
            },
            PurgeClaim {
                owner: &mcp,
                schemas: &mcp_schemas,
                declarations: mcp_decl,
            },
        ])
    };
    assert_eq!(
        claims(&foreign, &[]).unwrap_err(),
        PurgeAdmissionError::ForeignTable {
            member: users.clone(),
            table: "mcp_artifacts".to_owned(),
        }
    );
    let admitted = claims(&[], &foreign).expect("control: the owner declares its own table");
    assert_eq!(admitted.entries()[0].owner, mcp);
}

#[test]
fn an_orphan_sweep_needs_both_its_tables_owned_by_the_declarer() {
    let files = ExtensionId::new("files");
    let mcp = ExtensionId::new("mcp");
    let files_schemas = schemas(&["blobs", "blob_refs"]);
    let mcp_schemas = schemas(&["mcp_artifacts"]);
    let sweep = |referenced_by: &'static str| {
        [PurgeDeclaration::from(OrphanSweep {
            table: "blobs",
            key: "sha256",
            referenced_by,
            via: "sha256",
            user_column: "user_id",
        })]
    };
    let admit = |declarations: &[PurgeDeclaration]| {
        AdmittedPurge::admit(&[
            PurgeClaim {
                owner: &files,
                schemas: &files_schemas,
                declarations,
            },
            PurgeClaim {
                owner: &mcp,
                schemas: &mcp_schemas,
                declarations: &[],
            },
        ])
    };
    assert_eq!(
        admit(&sweep("mcp_artifacts")).unwrap_err(),
        PurgeAdmissionError::ForeignTable {
            member: files.clone(),
            table: "mcp_artifacts".to_owned(),
        }
    );
    admit(&sweep("blob_refs")).expect("control: a sweep over its own referrer is admitted");
}
