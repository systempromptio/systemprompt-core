//! Admission of user-purge declarations: a member may purge only its own
//! tables (a core member also another core member's), and a predicate is a
//! typed column-and-values narrowing, never SQL text.

use systemprompt_extension::{
    AdmittedPurge, OrphanSweep, PurgeAdmissionError, PurgeClaim, PurgeDeclaration, PurgeFilter,
    PurgePredicate, SchemaDefinition, StandingTag, UserPurgeTable,
};
use systemprompt_identifiers::ExtensionId;

fn schemas(tables: &[&str]) -> Vec<SchemaDefinition> {
    tables
        .iter()
        .map(|t| SchemaDefinition::new(*t, format!("CREATE TABLE {t} (user_id TEXT)")))
        .collect()
}

fn table(name: &'static str) -> PurgeDeclaration {
    PurgeDeclaration::from(UserPurgeTable {
        table: name,
        column: "user_id",
        predicate: None,
    })
}

fn admit(
    members: &[(&ExtensionId, StandingTag, &[SchemaDefinition], &[PurgeDeclaration])],
) -> Result<AdmittedPurge, PurgeAdmissionError> {
    let claims: Vec<PurgeClaim<'_>> = members
        .iter()
        .map(|(owner, standing, schemas, declarations)| PurgeClaim {
            owner,
            standing: *standing,
            schemas,
            declarations,
        })
        .collect();
    AdmittedPurge::admit(&claims)
}

#[test]
fn an_install_declaring_another_members_table_is_refused() {
    let users = ExtensionId::new("users");
    let install = ExtensionId::new("acme");
    let users_schemas = schemas(&["users"]);
    let install_schemas = schemas(&["acme_notes"]);
    let refused = admit(&[
        (&users, StandingTag::Core, &users_schemas, &[]),
        (&install, StandingTag::Install, &install_schemas, &[table("users")]),
    ]);
    assert_eq!(
        refused.unwrap_err(),
        PurgeAdmissionError::ForeignTable {
            member: install.clone(),
            table: "users".to_owned(),
        }
    );

    let admitted = admit(&[
        (&users, StandingTag::Core, &users_schemas, &[]),
        (&install, StandingTag::Install, &install_schemas, &[table("acme_notes")]),
    ])
    .expect("an install purging its own table is admitted");
    assert_eq!(admitted.entries().len(), 1);
    assert_eq!(admitted.entries()[0].owner, install);
    assert_eq!(admitted.members(), &[users, install]);
}

#[test]
fn a_table_no_member_declares_is_refused_even_for_a_core_member() {
    let users = ExtensionId::new("users");
    let users_schemas = schemas(&["users"]);
    let refused = admit(&[(&users, StandingTag::Core, &users_schemas, &[table("oauth_clients")])]);
    assert!(matches!(
        refused,
        Err(PurgeAdmissionError::ForeignTable { table, .. }) if table == "oauth_clients"
    ));
}

#[test]
fn a_core_member_may_purge_a_core_peers_table_and_an_install_may_not() {
    let users = ExtensionId::new("users");
    let mcp = ExtensionId::new("mcp");
    let install = ExtensionId::new("acme");
    let users_schemas = schemas(&["users"]);
    let mcp_schemas = schemas(&["mcp_artifacts"]);
    let install_schemas = schemas(&["acme_notes"]);
    admit(&[
        (&users, StandingTag::Core, &users_schemas, &[table("mcp_artifacts")]),
        (&mcp, StandingTag::Core, &mcp_schemas, &[]),
    ])
    .expect("core reach over a core peer's table");
    let refused = admit(&[
        (&mcp, StandingTag::Core, &mcp_schemas, &[]),
        (&install, StandingTag::Install, &install_schemas, &[table("mcp_artifacts")]),
    ]);
    assert!(matches!(refused, Err(PurgeAdmissionError::ForeignTable { member, .. }) if member == install));
}

#[test]
fn a_core_member_may_not_reach_an_install_members_table() {
    let users = ExtensionId::new("users");
    let install = ExtensionId::new("acme");
    let users_schemas = schemas(&["users"]);
    let install_schemas = schemas(&["acme_notes"]);
    let refused = admit(&[
        (&users, StandingTag::Core, &users_schemas, &[table("acme_notes")]),
        (&install, StandingTag::Install, &install_schemas, &[]),
    ]);
    assert!(matches!(refused, Err(PurgeAdmissionError::ForeignTable { member, .. }) if member == users));
}

#[test]
fn an_orphan_sweep_reading_a_foreign_referrer_is_refused() {
    let users = ExtensionId::new("users");
    let install = ExtensionId::new("acme");
    let users_schemas = schemas(&["users"]);
    let install_schemas = schemas(&["acme_blobs", "acme_refs"]);
    let sweep = |referenced_by: &'static str| {
        PurgeDeclaration::from(OrphanSweep {
            table: "acme_blobs",
            key: "sha256",
            referenced_by,
            via: "blob_sha256",
            user_column: "user_id",
        })
    };
    let refused = admit(&[
        (&users, StandingTag::Core, &users_schemas, &[]),
        (&install, StandingTag::Install, &install_schemas, &[sweep("users")]),
    ]);
    assert!(matches!(
        refused,
        Err(PurgeAdmissionError::ForeignTable { table, .. }) if table == "users"
    ));
    admit(&[(&install, StandingTag::Install, &install_schemas, &[sweep("acme_refs")])])
        .expect("a sweep over its own tables is admitted");
}

#[test]
fn a_table_in_a_non_public_schema_does_not_own_the_public_name() {
    let install = ExtensionId::new("acme");
    let mut owned = schemas(&["users"]);
    owned[0].schema = Some("acme".to_owned());
    let refused = admit(&[(&install, StandingTag::Install, &owned, &[table("users")])]);
    assert!(matches!(refused, Err(PurgeAdmissionError::ForeignTable { .. })));
}

#[test]
fn an_empty_predicate_is_refused() {
    let install = ExtensionId::new("acme");
    let owned = schemas(&["acme_notes"]);
    let empty = PurgeDeclaration::from(UserPurgeTable {
        table: "acme_notes",
        column: "user_id",
        predicate: Some(PurgePredicate {
            column: "kind",
            any_of: &[],
        }),
    });
    assert_eq!(
        admit(&[(&install, StandingTag::Install, &owned, &[empty])]).unwrap_err(),
        PurgeAdmissionError::EmptyPredicate {
            member: install.clone(),
            table: "acme_notes".to_owned(),
        }
    );
    let narrowed = PurgeDeclaration::from(UserPurgeTable {
        table: "acme_notes",
        column: "user_id",
        predicate: Some(PurgePredicate {
            column: "kind",
            any_of: &["user"],
        }),
    });
    admit(&[(&install, StandingTag::Install, &owned, &[narrowed])])
        .expect("a predicate with values is admitted");
}

#[test]
fn a_predicate_carries_values_as_data_and_its_column_as_a_name() {
    let declared = PurgeDeclaration::from(UserPurgeTable {
        table: "acme_notes",
        column: "user_id",
        predicate: Some(PurgePredicate {
            column: "kind",
            any_of: &["1=1) OR (1=1"],
        }),
    });
    let PurgeDeclaration::Table { predicate, .. } = &declared else {
        panic!("a table declaration");
    };
    assert_eq!(
        predicate.as_ref(),
        Some(&PurgeFilter {
            column: "kind".to_owned(),
            any_of: vec!["1=1) OR (1=1".to_owned()],
        })
    );
    assert_eq!(declared.identifiers(), vec!["acme_notes", "user_id", "kind"]);
}

#[test]
fn a_declaration_round_trips_through_its_recorded_form() {
    let declarations = vec![
        PurgeDeclaration::from(UserPurgeTable {
            table: "document_grants",
            column: "subject_value",
            predicate: Some(PurgePredicate {
                column: "subject_kind",
                any_of: &["user"],
            }),
        }),
        PurgeDeclaration::from(OrphanSweep {
            table: "artifact_payloads",
            key: "sha256",
            referenced_by: "mcp_artifacts",
            via: "payload_sha256",
            user_column: "user_id",
        }),
    ];
    let recorded = serde_json::to_string(&declarations).expect("encode");
    let read: Vec<PurgeDeclaration> = serde_json::from_str(&recorded).expect("decode");
    assert_eq!(read, declarations);
}
