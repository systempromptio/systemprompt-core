//! Admission of owner reassignments: a member may reassign only columns of
//! tables its own schema creates, named by plain identifiers.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{
    AdmittedOwnerReassignments, ReassignmentAdmissionError, ReassignmentClaim, SchemaDefinition,
};
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_traits::{
    DynOwnerReassignment, OwnerReassignment, ReassignedColumn, ReassignedRows, RepositoryError,
};

struct Probe(&'static [ReassignedColumn]);

#[async_trait]
impl OwnerReassignment for Probe {
    fn domain(&self) -> &'static str {
        "probe"
    }

    fn columns(&self) -> &'static [ReassignedColumn] {
        self.0
    }

    async fn reassign_owner(
        &self,
        _from: &UserId,
        _to: &UserId,
    ) -> Result<ReassignedRows, RepositoryError> {
        Ok(ReassignedRows::default())
    }
}

const OWN: &[ReassignedColumn] = &[ReassignedColumn {
    table: "acme_notes",
    column: "user_id",
}];
const FOREIGN: &[ReassignedColumn] = &[ReassignedColumn {
    table: "users",
    column: "id",
}];
const INJECTED: &[ReassignedColumn] = &[ReassignedColumn {
    table: "acme_notes",
    column: "user_id = user_id; DROP TABLE users; --",
}];

fn schemas(tables: &[&str]) -> Vec<SchemaDefinition> {
    tables
        .iter()
        .map(|t| SchemaDefinition::new(*t, format!("CREATE TABLE {t} (user_id TEXT)")))
        .collect()
}

fn admit(
    owner: &ExtensionId,
    schemas: &[SchemaDefinition],
    columns: &'static [ReassignedColumn],
) -> Result<AdmittedOwnerReassignments, ReassignmentAdmissionError> {
    let reassignment: DynOwnerReassignment = Arc::new(Probe(columns));
    let reassignments = [reassignment];
    AdmittedOwnerReassignments::admit(&[ReassignmentClaim {
        owner,
        schemas,
        reassignments: &reassignments,
    }])
}

#[test]
fn a_reassignment_over_another_members_table_is_refused() {
    let install = ExtensionId::new("acme");
    let error = admit(&install, &schemas(&["acme_notes"]), FOREIGN)
        .expect_err("users belongs to the users member");
    assert!(
        matches!(
            &error,
            ReassignmentAdmissionError::ForeignTable { member, table }
                if member == &install && table == "users"
        ),
        "{error:?}"
    );
}

#[test]
fn a_reassignment_over_the_members_own_table_is_admitted() {
    let install = ExtensionId::new("acme");
    let admitted = admit(&install, &schemas(&["acme_notes"]), OWN)
        .expect("negative control: acme_notes is the member's own");
    assert_eq!(admitted.entries().len(), 1);
    assert_eq!(admitted.entries()[0].owner, install);
}

#[test]
fn a_column_that_is_not_a_plain_identifier_is_refused() {
    let install = ExtensionId::new("acme");
    let error = admit(&install, &schemas(&["acme_notes"]), INJECTED)
        .expect_err("a column name is an identifier, never SQL");
    assert!(
        matches!(error, ReassignmentAdmissionError::InvalidColumn { .. }),
        "{error:?}"
    );
}

#[test]
fn a_reassignment_naming_no_column_is_refused() {
    let install = ExtensionId::new("acme");
    let error = admit(&install, &schemas(&["acme_notes"]), &[])
        .expect_err("a reassignment declares what it rewrites");
    assert!(
        matches!(error, ReassignmentAdmissionError::NoColumns { .. }),
        "{error:?}"
    );
}
