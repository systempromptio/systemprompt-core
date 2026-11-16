//! Every admitted member's user-purge tables and orphan sweeps, stamped with
//! their owners in composition order.
//!
//! The kernel builds one [`AdmittedPurge`] from the admitted composition and
//! seeds it into state; the users domain's delete, purge and dry run take it
//! as an argument. Construction is admission: a declaration naming a table
//! outside the declaring member's own schema tables is refused with
//! [`PurgeAdmissionError::ForeignTable`]. A core member may also name a table
//! another core member's schema declares, which is how the users member
//! purges the core activity tables; an install member may name only its own.
//! The installer admits the same claims before it records them, so a recorded
//! declaration of a member the profile has since disabled passed this check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::ExtensionId;
use thiserror::Error;

use crate::admitted::BuiltState;
use crate::contribute::{ContributionSink, StandingTag};
use crate::metadata::SchemaDefinition;
use crate::purge::PurgeDeclaration;

/// Why a member's purge declarations were refused at admission.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PurgeAdmissionError {
    #[error(
        "member '{member}' declares a user purge over table '{table}', which is not among its own \
         schema tables"
    )]
    ForeignTable { member: ExtensionId, table: String },
    #[error("member '{member}' declares a purge predicate over table '{table}' with no values")]
    EmptyPredicate { member: ExtensionId, table: String },
}

/// One member's purge declarations with what admission judges them by.
#[derive(Debug, Clone, Copy)]
pub struct PurgeClaim<'a> {
    pub owner: &'a ExtensionId,
    pub standing: StandingTag,
    pub schemas: &'a [SchemaDefinition],
    pub declarations: &'a [PurgeDeclaration],
}

/// One admitted declaration with its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedPurge {
    pub owner: ExtensionId,
    pub declaration: PurgeDeclaration,
}

/// The owner-stamped purge declarations of an admitted composition, and
/// every admitted member's id.
#[derive(Debug, Clone, Default)]
pub struct AdmittedPurge {
    members: Vec<ExtensionId>,
    entries: Vec<OwnedPurge>,
}

#[must_use]
pub fn purge_declarations(sink: &ContributionSink) -> Vec<PurgeDeclaration> {
    sink.user_purges()
        .iter()
        .map(|table| PurgeDeclaration::from(*table))
        .chain(sink.orphan_sweeps().iter().map(|sweep| PurgeDeclaration::from(*sweep)))
        .collect()
}

fn own_tables(schemas: &[SchemaDefinition]) -> impl Iterator<Item = &str> {
    schemas
        .iter()
        .filter(|schema| schema.schema_name() == "public")
        .filter_map(|schema| schema.table.as_deref())
}

impl AdmittedPurge {
    pub fn of(built: &BuiltState) -> Result<Self, PurgeAdmissionError> {
        let declarations: Vec<Vec<PurgeDeclaration>> = built
            .members()
            .map(|member| purge_declarations(&member.sink))
            .collect();
        let claims: Vec<PurgeClaim<'_>> = built
            .members()
            .zip(&declarations)
            .map(|(member, declarations)| PurgeClaim {
                owner: &member.owner,
                standing: member.standing,
                schemas: member.sink.schemas(),
                declarations,
            })
            .collect();
        Self::admit(&claims)
    }

    pub fn admit(claims: &[PurgeClaim<'_>]) -> Result<Self, PurgeAdmissionError> {
        let core_tables: BTreeSet<&str> = claims
            .iter()
            .filter(|claim| claim.standing == StandingTag::Core)
            .flat_map(|claim| own_tables(claim.schemas))
            .collect();
        let mut entries = Vec::new();
        for claim in claims {
            let own: BTreeSet<&str> = own_tables(claim.schemas).collect();
            for declaration in claim.declarations {
                check_declaration(claim, declaration, &own, &core_tables)?;
                entries.push(OwnedPurge {
                    owner: claim.owner.clone(),
                    declaration: declaration.clone(),
                });
            }
        }
        Ok(Self {
            members: claims.iter().map(|claim| claim.owner.clone()).collect(),
            entries,
        })
    }

    #[must_use]
    pub fn members(&self) -> &[ExtensionId] {
        &self.members
    }

    #[must_use]
    pub fn contains(&self, owner: &ExtensionId) -> bool {
        self.members.contains(owner)
    }

    #[must_use]
    pub fn entries(&self) -> &[OwnedPurge] {
        &self.entries
    }
}

fn check_declaration(
    claim: &PurgeClaim<'_>,
    declaration: &PurgeDeclaration,
    own: &BTreeSet<&str>,
    core_tables: &BTreeSet<&str>,
) -> Result<(), PurgeAdmissionError> {
    for table in declaration.tables() {
        let core_reach = claim.standing == StandingTag::Core && core_tables.contains(table);
        if !own.contains(table) && !core_reach {
            return Err(PurgeAdmissionError::ForeignTable {
                member: claim.owner.clone(),
                table: table.to_owned(),
            });
        }
    }
    if let PurgeDeclaration::Table {
        table,
        predicate: Some(filter),
        ..
    } = declaration
        && filter.any_of.is_empty()
    {
        return Err(PurgeAdmissionError::EmptyPredicate {
            member: claim.owner.clone(),
            table: table.clone(),
        });
    }
    Ok(())
}
