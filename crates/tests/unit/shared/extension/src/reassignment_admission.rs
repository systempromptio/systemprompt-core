//! Admission of owner reassignments: a member may declare only columns of
//! tables its own schema creates, and every name it declares is a plain
//! identifier the kernel renders, never SQL.

use systemprompt_extension::{
    AdmittedOwnerReassignments, OwnerReassignment, PurgePredicate, ReassignAction,
    ReassignmentAdmissionError, ReassignmentClaim, SchemaDefinition,
};
use systemprompt_identifiers::ExtensionId;

fn schemas(tables: &[&str]) -> Vec<SchemaDefinition> {
    tables
        .iter()
        .map(|t| SchemaDefinition::new(*t, format!("CREATE TABLE {t} (user_id TEXT)")))
        .collect()
}

fn admit(
    owner: &ExtensionId,
    schemas: &[SchemaDefinition],
    reassignment: OwnerReassignment,
) -> Result<AdmittedOwnerReassignments, ReassignmentAdmissionError> {
    AdmittedOwnerReassignments::admit(&[ReassignmentClaim {
        owner,
        schemas,
        reassignments: &[reassignment],
    }])
}

const fn update(table: &'static str, column: &'static str) -> OwnerReassignment {
    OwnerReassignment {
        table,
        column,
        action: ReassignAction::UPDATE,
    }
}

#[test]
fn a_reassignment_over_another_members_table_is_refused() {
    let install = ExtensionId::new("acme");
    let error = admit(&install, &schemas(&["acme_notes"]), update("users", "id"))
        .expect_err("users belongs to the users member");
    assert_eq!(
        error,
        ReassignmentAdmissionError::ForeignTable {
            member: install,
            table: "users".to_owned(),
        }
    );
}

#[test]
fn a_reassignment_over_the_members_own_table_is_admitted_as_declared() {
    let install = ExtensionId::new("acme");
    let declared = update("acme_notes", "user_id");
    let admitted = admit(&install, &schemas(&["acme_notes"]), declared)
        .expect("negative control: acme_notes is the member's own");
    assert_eq!(admitted.entries().len(), 1);
    assert_eq!(admitted.entries()[0].owner, install);
    assert_eq!(admitted.entries()[0].reassignment, declared);
}

#[test]
fn every_declared_name_must_be_a_plain_identifier() {
    let install = ExtensionId::new("acme");
    let injected = "user_id = user_id; DROP TABLE users; --";
    for declared in [
        update("acme_notes", injected),
        OwnerReassignment {
            table: "acme_notes",
            column: "user_id",
            action: ReassignAction::Update {
                touch: Some(injected),
                keep_target_on: &[],
            },
        },
        OwnerReassignment {
            table: "acme_notes",
            column: "user_id",
            action: ReassignAction::Update {
                touch: None,
                keep_target_on: &["ok", "user_id) OR (1=1"],
            },
        },
        OwnerReassignment {
            table: "acme_notes",
            column: "user_id",
            action: ReassignAction::Delete {
                predicate: Some(PurgePredicate {
                    column: "kind OR 1=1",
                    any_of: &["user"],
                }),
            },
        },
    ] {
        let error = admit(&install, &schemas(&["acme_notes"]), declared)
            .expect_err("a declared name is an identifier, never SQL");
        assert!(
            matches!(error, ReassignmentAdmissionError::InvalidIdentifier { .. }),
            "{declared:?}: {error:?}"
        );
    }
}

#[test]
fn a_delete_with_an_empty_predicate_is_refused() {
    let install = ExtensionId::new("acme");
    let declared = OwnerReassignment {
        table: "acme_notes",
        column: "user_id",
        action: ReassignAction::Delete {
            predicate: Some(PurgePredicate {
                column: "kind",
                any_of: &[],
            }),
        },
    };
    let error = admit(&install, &schemas(&["acme_notes"]), declared)
        .expect_err("an empty predicate matches nothing it declares");
    assert!(
        matches!(error, ReassignmentAdmissionError::EmptyPredicate { .. }),
        "{error:?}"
    );
}
