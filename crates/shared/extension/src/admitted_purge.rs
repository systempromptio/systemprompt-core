//! Every admitted member's user-purge tables and orphan sweeps, stamped with
//! their owners in composition order.
//!
//! The kernel builds one [`AdmittedPurge`] from the admitted composition and
//! seeds it into state; the users domain's delete, purge and dry run take it
//! as an argument. Construction is admission: a declaration naming a table
//! outside the declaring member's own schema tables is refused with
//! [`PurgeAdmissionError::ForeignTable`], whatever the member's standing.
//! Every member declares the purge of its own tables, so a composition that
//! leaves a member out leaves out exactly that member's declarations. An
//! orphan sweep is judged by both of its tables: the swept table and the
//! table whose references keep a row alive must both be the declarer's own,
//! so a sweep never runs while the member that writes its references is
//! absent. The installer admits the same claims before it records them, so a
//! recorded declaration of a member the profile has since disabled passed
//! this check.
//!
//! A member's own tables are the `public` tables its schema SQL creates, read
//! by [`created_table_names`] — the parser the installer records table
//! ownership with — never the self-declared [`SchemaDefinition::table`]
//! label, so a schema labelled with a core table's name owns only what its
//! SQL creates. Schema SQL that does not parse refuses admission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::ExtensionId;
use thiserror::Error;

use crate::admitted::BuiltState;
use crate::contribute::ContributionSink;
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
    #[error("schema SQL of member '{member}' does not parse")]
    UnparseableSchema {
        member: ExtensionId,
        #[source]
        source: pg_query::Error,
    },
}

/// One member's purge declarations with what admission judges them by.
#[derive(Debug, Clone, Copy)]
pub struct PurgeClaim<'a> {
    pub owner: &'a ExtensionId,
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
        .chain(
            sink.orphan_sweeps()
                .iter()
                .map(|sweep| PurgeDeclaration::from(*sweep)),
        )
        .collect()
}

pub fn created_table_names(sql: &str) -> Result<Vec<String>, pg_query::Error> {
    let parsed = pg_query::parse(sql)?;
    Ok(parsed
        .protobuf
        .stmts
        .iter()
        .filter_map(|raw| match raw.stmt.as_ref()?.node.as_ref()? {
            pg_query::protobuf::node::Node::CreateStmt(create) => {
                let relation = create.relation.as_ref()?;
                if relation.relname.is_empty() {
                    None
                } else if relation.schemaname.is_empty() {
                    Some(relation.relname.clone())
                } else {
                    Some(format!("{}.{}", relation.schemaname, relation.relname))
                }
            },
            _ => None,
        })
        .collect())
}

fn own_tables(
    owner: &ExtensionId,
    schemas: &[SchemaDefinition],
) -> Result<BTreeSet<String>, PurgeAdmissionError> {
    let mut own = BTreeSet::new();
    for schema in schemas
        .iter()
        .filter(|schema| schema.schema_name() == "public")
    {
        let created = created_table_names(&schema.sql).map_err(|source| {
            PurgeAdmissionError::UnparseableSchema {
                member: owner.clone(),
                source,
            }
        })?;
        own.extend(
            created
                .into_iter()
                .filter_map(|name| match name.split_once('.') {
                    None => Some(name),
                    Some(("public", table)) => Some(table.to_owned()),
                    Some(_) => None,
                }),
        );
    }
    Ok(own)
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
                schemas: member.sink.schemas(),
                declarations,
            })
            .collect();
        Self::admit(&claims)
    }

    pub fn admit(claims: &[PurgeClaim<'_>]) -> Result<Self, PurgeAdmissionError> {
        let owned: Vec<BTreeSet<String>> = claims
            .iter()
            .map(|claim| own_tables(claim.owner, claim.schemas))
            .collect::<Result<_, _>>()?;
        let mut entries = Vec::new();
        for (claim, own) in claims.iter().zip(&owned) {
            for declaration in claim.declarations {
                check_declaration(claim, declaration, own)?;
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
    own: &BTreeSet<String>,
) -> Result<(), PurgeAdmissionError> {
    for table in declaration.tables() {
        if !own.contains(table) {
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
