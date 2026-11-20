//! Every admitted member's user-owner reassignments, stamped with their
//! owners in composition order.
//!
//! A member that owns user-keyed rows contributes the reassignment of its own
//! rows with `Contributions::owner_reassignment`. The kernel builds each
//! contributed factory once the members' provide fns have run and admits the
//! built reassignment only when every [`ReassignedColumn`] it names is a
//! plain identifier on a table the contributing member's own schema SQL
//! creates (read by [`created_table_names`](crate::created_table_names), the
//! same check user purges pass); anything else is refused with
//! [`ReassignmentAdmissionError`], whatever the member's standing. No member
//! declares a reassignment for another member's tables, so a composition
//! that leaves a member out leaves out exactly that member's reassignment.
//!
//! The users domain's merge runs the admitted set in composition order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::{DynOwnerReassignment, ReassignedColumn, is_plain_identifier};
use thiserror::Error;

use crate::admitted::BuiltState;
use crate::admitted_purge::{PurgeAdmissionError, own_tables};
use crate::contribute::{FactoryError, Resolved};
use crate::metadata::SchemaDefinition;

/// Why a member's owner reassignment was refused at admission.
#[derive(Debug, Error)]
pub enum ReassignmentAdmissionError {
    #[error(
        "member '{member}' declares an owner reassignment over table '{table}', which is not \
         among its own schema tables"
    )]
    ForeignTable { member: ExtensionId, table: String },
    #[error(
        "member '{member}' declares an owner reassignment over '{table}.{column}', which is not a \
         plain identifier"
    )]
    InvalidColumn {
        member: ExtensionId,
        table: String,
        column: String,
    },
    #[error("member '{member}' declares an owner reassignment that names no column")]
    NoColumns { member: ExtensionId },
    #[error("owner reassignment factory of '{member}' failed")]
    Factory {
        member: ExtensionId,
        #[source]
        source: FactoryError,
    },
    #[error("schema tables of a reassigning member could not be read")]
    Schema(#[from] PurgeAdmissionError),
}

/// One member's built reassignments with what admission judges them by.
#[derive(Clone, Copy)]
pub struct ReassignmentClaim<'a> {
    pub owner: &'a ExtensionId,
    pub schemas: &'a [SchemaDefinition],
    pub reassignments: &'a [DynOwnerReassignment],
}

impl std::fmt::Debug for ReassignmentClaim<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReassignmentClaim")
            .field("owner", self.owner)
            .field("reassignments", &self.reassignments.len())
            .finish_non_exhaustive()
    }
}

/// One admitted reassignment with its owner.
#[derive(Clone)]
pub struct OwnedReassignment {
    pub owner: ExtensionId,
    pub reassignment: DynOwnerReassignment,
}

impl std::fmt::Debug for OwnedReassignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedReassignment")
            .field("owner", &self.owner)
            .field("domain", &self.reassignment.domain())
            .finish()
    }
}

/// The owner-stamped reassignments of an admitted composition.
#[derive(Debug, Clone, Default)]
pub struct AdmittedOwnerReassignments {
    entries: Vec<OwnedReassignment>,
}

impl AdmittedOwnerReassignments {
    pub fn of(built: &BuiltState, r: &Resolved<'_>) -> Result<Self, ReassignmentAdmissionError> {
        let reassignments: Vec<Vec<DynOwnerReassignment>> = built
            .members()
            .map(|member| {
                member
                    .sink
                    .owner_reassignments()
                    .iter()
                    .map(|factory| {
                        factory
                            .build(r)
                            .map_err(|source| ReassignmentAdmissionError::Factory {
                                member: member.owner.clone(),
                                source,
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<_, _>>()?;
        let claims: Vec<ReassignmentClaim<'_>> = built
            .members()
            .zip(&reassignments)
            .map(|(member, reassignments)| ReassignmentClaim {
                owner: &member.owner,
                schemas: member.sink.schemas(),
                reassignments,
            })
            .collect();
        Self::admit(&claims)
    }

    pub fn admit(claims: &[ReassignmentClaim<'_>]) -> Result<Self, ReassignmentAdmissionError> {
        let mut entries = Vec::new();
        for claim in claims.iter().filter(|claim| !claim.reassignments.is_empty()) {
            let own = own_tables(claim.owner, claim.schemas)?;
            for reassignment in claim.reassignments {
                check_columns(claim.owner, reassignment.columns(), &own)?;
                entries.push(OwnedReassignment {
                    owner: claim.owner.clone(),
                    reassignment: reassignment.clone(),
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
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn check_columns(
    owner: &ExtensionId,
    columns: &[ReassignedColumn],
    own: &BTreeSet<String>,
) -> Result<(), ReassignmentAdmissionError> {
    if columns.is_empty() {
        return Err(ReassignmentAdmissionError::NoColumns {
            member: owner.clone(),
        });
    }
    for column in columns {
        if !is_plain_identifier(column.table) || !is_plain_identifier(column.column) {
            return Err(ReassignmentAdmissionError::InvalidColumn {
                member: owner.clone(),
                table: column.table.to_owned(),
                column: column.column.to_owned(),
            });
        }
        if !own.contains(column.table) {
            return Err(ReassignmentAdmissionError::ForeignTable {
                member: owner.clone(),
                table: column.table.to_owned(),
            });
        }
    }
    Ok(())
}
