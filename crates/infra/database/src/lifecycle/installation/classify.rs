//! Phase classification of one declarative-schema statement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

use super::fk_deferral::{FkDeferralError, SplitCreateTable, split_foreign_keys};

#[derive(Debug, Error)]
pub(super) enum ClassifyError {
    #[error("SQL parse failed: {source}\nSQL:\n{statement}")]
    Parse {
        statement: String,
        #[source]
        source: pg_query::Error,
    },

    #[error(
        "declarative functions must be CREATE OR REPLACE: the installer applies them before \
         migrations and again after\nSQL:\n{statement}"
    )]
    FunctionNotReplace { statement: String },

    #[error("{source}\nSQL:\n{statement}")]
    ForeignKeys {
        statement: String,
        #[source]
        source: FkDeferralError,
    },

    #[error(
        "unrecognised statement type {kind} in declarative schema; classify it as structural or \
         dependent in classify_statement()\nSQL:\n{statement}"
    )]
    UnrecognisedStatement { kind: String, statement: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatementPhase {
    Structural,
    Dependent,
}

pub(super) enum Classified {
    Structural,
    Routine,
    Dependent,
    CreateTable(SplitCreateTable),
}

const fn node_phase(node: &pg_query::NodeEnum) -> Option<StatementPhase> {
    use pg_query::NodeEnum;

    Some(match node {
        NodeEnum::CreateSchemaStmt(_)
        | NodeEnum::CreateStmt(_)
        | NodeEnum::CreateExtensionStmt(_)
        | NodeEnum::CompositeTypeStmt(_)
        | NodeEnum::CreateEnumStmt(_)
        | NodeEnum::CreateRangeStmt(_)
        | NodeEnum::CreateSeqStmt(_)
        | NodeEnum::CreateDomainStmt(_)
        | NodeEnum::DefineStmt(_)
        | NodeEnum::CreateForeignTableStmt(_) => StatementPhase::Structural,

        NodeEnum::IndexStmt(_)
        | NodeEnum::ViewStmt(_)
        | NodeEnum::CreateTableAsStmt(_)
        | NodeEnum::CreateTrigStmt(_)
        | NodeEnum::CreateFunctionStmt(_)
        | NodeEnum::CreatePolicyStmt(_)
        | NodeEnum::AlterPolicyStmt(_)
        | NodeEnum::RuleStmt(_)
        | NodeEnum::CreateStatsStmt(_)
        | NodeEnum::CreateCastStmt(_)
        | NodeEnum::CreateTransformStmt(_)
        | NodeEnum::AlterTableStmt(_)
        | NodeEnum::AlterEnumStmt(_)
        | NodeEnum::AlterSeqStmt(_)
        | NodeEnum::AlterDomainStmt(_)
        | NodeEnum::AlterOwnerStmt(_)
        | NodeEnum::AlterObjectSchemaStmt(_)
        | NodeEnum::RenameStmt(_)
        | NodeEnum::GrantStmt(_)
        | NodeEnum::GrantRoleStmt(_)
        | NodeEnum::CommentStmt(_)
        | NodeEnum::DropStmt(_) => StatementPhase::Dependent,

        _ => return None,
    })
}

pub(super) fn classify_statement(statement: &str) -> Result<Classified, ClassifyError> {
    use pg_query::NodeEnum;

    let parsed = pg_query::parse(statement).map_err(|source| ClassifyError::Parse {
        statement: statement.to_owned(),
        source,
    })?;

    let mut phase: Option<StatementPhase> = None;
    let mut create_table: Option<SplitCreateTable> = None;
    let mut routine = false;
    for raw in parsed.protobuf.stmts {
        let Some(node) = raw.stmt.and_then(|s| s.node) else {
            continue;
        };
        if let NodeEnum::CreateFunctionStmt(create) = &node {
            if !create.replace {
                return Err(ClassifyError::FunctionNotReplace {
                    statement: statement.to_owned(),
                });
            }
            routine = true;
        }
        if let NodeEnum::CreateStmt(create) = &node
            && create_table.is_none()
        {
            create_table = Some(split_foreign_keys(statement, create).map_err(|source| {
                ClassifyError::ForeignKeys {
                    statement: statement.to_owned(),
                    source,
                }
            })?);
        }
        let Some(node_phase) = node_phase(&node) else {
            return Err(ClassifyError::UnrecognisedStatement {
                kind: format!("{node:?}"),
                statement: statement.to_owned(),
            });
        };
        phase = Some(match phase {
            None | Some(StatementPhase::Structural) => node_phase,
            Some(StatementPhase::Dependent) => StatementPhase::Dependent,
        });
    }

    Ok(
        match (phase.unwrap_or(StatementPhase::Dependent), create_table) {
            (StatementPhase::Structural, Some(split)) => Classified::CreateTable(split),
            (StatementPhase::Structural, None) => Classified::Structural,
            (StatementPhase::Dependent, _) if routine => Classified::Routine,
            (StatementPhase::Dependent, _) => Classified::Dependent,
        },
    )
}
