//! Every admitted member's owner reassignments, stamped with their owners in
//! composition order.
//!
//! A member that owns user-keyed rows declares each reassigned column with
//! `Contributions::owner_reassignment` as an [`OwnerReassignment`]: a table,
//! a column and an action, never code. The kernel admits a declaration only
//! when every name it carries is a plain identifier and its table is one the
//! contributing member's own schema SQL creates (read by
//! [`created_table_names`](crate::created_table_names), the same check user
//! purges pass); anything else is refused with [`ReassignmentAdmissionError`],
//! whatever the member's standing. No member declares a reassignment for
//! another member's tables, so a composition that leaves a member out leaves
//! out exactly that member's reassignments.
//!
//! The users domain's merge renders each admitted declaration into its
//! statement and runs them all in one transaction, in composition order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::is_plain_identifier;
use thiserror::Error;

use crate::admitted::BuiltState;
use crate::admitted_purge::{PurgeAdmissionError, own_tables};
use crate::metadata::SchemaDefinition;
use crate::reassignment::{OwnerReassignment, ReassignAction};

/// Why a member's owner reassignment was refused at admission.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReassignmentAdmissionError {
    #[error(
        "member '{member}' declares an owner reassignment over table '{table}', which is not \
         among its own schema tables"
    )]
    ForeignTable { member: ExtensionId, table: String },
    #[error(
        "member '{member}' declares an owner reassignment over table '{table}' naming '{name}', \
         which is not a plain identifier"
    )]
    InvalidIdentifier {
        member: ExtensionId,
        table: String,
        name: String,
    },
    #[error(
        "member '{member}' declares an owner reassignment over table '{table}' with an empty \
         predicate"
    )]
    EmptyPredicate { member: ExtensionId, table: String },
    #[error("schema tables of a reassigning member could not be read")]
    Schema(#[from] PurgeAdmissionError),
}

/// One member's reassignment declarations with what admission judges them by.
#[derive(Debug, Clone, Copy)]
pub struct ReassignmentClaim<'a> {
    pub owner: &'a ExtensionId,
    pub schemas: &'a [SchemaDefinition],
    pub reassignments: &'a [OwnerReassignment],
}

/// One admitted reassignment with its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedReassignment {
    pub owner: ExtensionId,
    pub reassignment: OwnerReassignment,
}

/// The owner-stamped reassignments of an admitted composition.
#[derive(Debug, Clone, Default)]
pub struct AdmittedOwnerReassignments {
    entries: Vec<OwnedReassignment>,
}

impl AdmittedOwnerReassignments {
    pub fn of(built: &BuiltState) -> Result<Self, ReassignmentAdmissionError> {
        let claims: Vec<ReassignmentClaim<'_>> = built
            .members()
            .map(|member| ReassignmentClaim {
                owner: &member.owner,
                schemas: member.sink.schemas(),
                reassignments: member.sink.owner_reassignments(),
            })
            .collect();
        Self::admit(&claims)
    }

    pub fn admit(claims: &[ReassignmentClaim<'_>]) -> Result<Self, ReassignmentAdmissionError> {
        let mut entries = Vec::new();
        for claim in claims
            .iter()
            .filter(|claim| !claim.reassignments.is_empty())
        {
            let own = own_tables(claim.owner, claim.schemas)?;
            for reassignment in claim.reassignments {
                check_declaration(claim.owner, reassignment, &own)?;
                entries.push(OwnedReassignment {
                    owner: claim.owner.clone(),
                    reassignment: *reassignment,
                });
            }
        }
        Ok(Self { entries })
    }

    #[must_use]
    pub fn entries(&self) -> &[OwnedReassignment] {
        &self.entries
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn check_declaration(
    owner: &ExtensionId,
    reassignment: &OwnerReassignment,
    own: &BTreeSet<String>,
) -> Result<(), ReassignmentAdmissionError> {
    let table = reassignment.table;
    if let Some(name) = reassignment
        .identifiers()
        .into_iter()
        .find(|name| !is_plain_identifier(name))
    {
        return Err(ReassignmentAdmissionError::InvalidIdentifier {
            member: owner.clone(),
            table: table.to_owned(),
            name: name.to_owned(),
        });
    }
    if !own.contains(table) {
        return Err(ReassignmentAdmissionError::ForeignTable {
            member: owner.clone(),
            table: table.to_owned(),
        });
    }
    if let ReassignAction::Delete {
        predicate: Some(predicate),
    } = reassignment.action
        && predicate.any_of.is_empty()
    {
        return Err(ReassignmentAdmissionError::EmptyPredicate {
            member: owner.clone(),
            table: table.to_owned(),
        });
    }
    Ok(())
}
