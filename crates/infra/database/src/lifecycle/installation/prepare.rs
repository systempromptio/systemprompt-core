//! Parse, lint, and phase-classify an extension's declarative schema before
//! any database I/O. The resulting [`PreparedSchema`] is executed by the
//! installer in the correct global phase.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{Extension, LoaderError};
use tracing::warn;

use super::fk_deferral::{DeferredForeignKey, SplitCreateTable, split_foreign_keys};
use crate::services::SqlExecutor;
use crate::services::schema_linter::{created_table_names, lint_declarative_schemas};

pub(super) struct PreparedSchema {
    pub(super) extension_id: String,
    pub(super) structural: Vec<String>,
    pub(super) dependent: Vec<String>,
    pub(super) foreign_keys: Vec<DeferredForeignKey>,
    pub(super) columns_to_validate: Vec<ColumnsToValidate>,
    pub(super) owned_tables: Vec<String>,
}

pub(super) struct ColumnsToValidate {
    pub(super) schema: String,
    pub(super) table: String,
    pub(super) columns: Vec<String>,
}

pub(super) fn prepare_extension_schema(ext: &dyn Extension) -> Result<PreparedSchema, LoaderError> {
    let schemas = ext.schemas();
    let extension_id = ext.metadata().id.to_owned();

    let mut all_sql = Vec::new();
    let mut columns_to_validate: Vec<ColumnsToValidate> = Vec::new();

    let lint_errors = lint_schemas(&extension_id, &schemas);

    for schema in &schemas {
        all_sql.push(schema.sql.as_str());

        if let Some(table) = schema.table.as_ref()
            && !schema.required_columns.is_empty()
        {
            columns_to_validate.push(ColumnsToValidate {
                schema: schema.schema_name().to_owned(),
                table: table.clone(),
                columns: schema.required_columns.clone(),
            });
        }
    }

    require_declarative_schema(&extension_id, &lint_errors)?;

    let combined = all_sql.join("\n");
    let parse_failed = |e: &dyn std::fmt::Display| LoaderError::SchemaInstallationFailed {
        extension: extension_id.clone(),
        message: format!("SQL parse failed: {e}"),
    };
    let owned_tables = created_table_names(&combined).map_err(|e| parse_failed(&e))?;
    let parsed = SqlExecutor::parse_sql_statements(&combined).map_err(|e| parse_failed(&e))?;

    let Phased {
        structural,
        dependent,
        foreign_keys,
    } = phase_statements(&extension_id, parsed)?;

    Ok(PreparedSchema {
        extension_id,
        structural,
        dependent,
        foreign_keys,
        columns_to_validate,
        owned_tables,
    })
}

// Why: one lint call over every file, so a foreign key in one file is
// checked against the table another file of the same extension declares.
fn lint_schemas(
    extension_id: &str,
    schemas: &[systemprompt_extension::SchemaDefinition],
) -> Vec<String> {
    let lint_inputs: Vec<(&str, &str)> = schemas
        .iter()
        .map(|schema| {
            (
                schema.table.as_deref().unwrap_or(extension_id),
                schema.sql.as_str(),
            )
        })
        .collect();
    match lint_declarative_schemas(&lint_inputs) {
        Ok(warnings) => {
            for warning in &warnings {
                warn!(
                    extension = %extension_id,
                    source = %warning.source,
                    line = warning.line,
                    column = warning.column,
                    finding = %warning.message,
                    "Declarative schema lint warning"
                );
            }
            Vec::new()
        },
        Err(findings) => findings.iter().map(ToString::to_string).collect(),
    }
}

struct Phased {
    structural: Vec<String>,
    dependent: Vec<String>,
    foreign_keys: Vec<DeferredForeignKey>,
}

fn phase_statements(extension_id: &str, parsed: Vec<String>) -> Result<Phased, LoaderError> {
    let mut phased = Phased {
        structural: Vec::new(),
        dependent: Vec::new(),
        foreign_keys: Vec::new(),
    };
    for statement in parsed {
        let classified = classify_statement(&statement).map_err(|message| {
            LoaderError::SchemaInstallationFailed {
                extension: extension_id.to_owned(),
                message,
            }
        })?;
        match classified {
            Classified::Structural => phased.structural.push(statement),
            Classified::Dependent => phased.dependent.push(statement),
            Classified::CreateTable(split) => {
                phased.structural.push(split.create_table_sql);
                phased.foreign_keys.extend(split.foreign_keys);
            },
        }
    }
    Ok(phased)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatementPhase {
    Structural,
    Dependent,
}

fn require_declarative_schema(
    extension_id: &str,
    lint_errors: &[String],
) -> Result<(), LoaderError> {
    if lint_errors.is_empty() {
        return Ok(());
    }
    Err(LoaderError::SchemaInstallationFailed {
        extension: extension_id.to_owned(),
        message: format!(
            "Imperative SQL detected in declarative schema. Move offending statements to \
             schema/migrations/NNN_<name>.sql and declare them via \
             Extension::migrations():\n{}",
            lint_errors.join("\n")
        ),
    })
}

enum Classified {
    Structural,
    Dependent,
    CreateTable(SplitCreateTable),
}

fn classify_statement(statement: &str) -> Result<Classified, String> {
    use pg_query::NodeEnum;

    let parsed = pg_query::parse(statement)
        .map_err(|e| format!("SQL parse failed: {e}\nSQL:\n{statement}"))?;

    let mut phase: Option<StatementPhase> = None;
    let mut create_table: Option<SplitCreateTable> = None;
    for raw in parsed.protobuf.stmts {
        let Some(node) = raw.stmt.and_then(|s| s.node) else {
            continue;
        };
        if let NodeEnum::CreateStmt(create) = &node
            && create_table.is_none()
        {
            create_table = Some(
                split_foreign_keys(statement, create)
                    .map_err(|e| format!("{e}\nSQL:\n{statement}"))?,
            );
        }
        let node_phase = match node {
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

            other => {
                return Err(format!(
                    "unrecognised statement type {other:?} in declarative schema; classify it as \
                     structural or dependent in classify_statement()\nSQL:\n{statement}"
                ));
            },
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
            (StatementPhase::Dependent, _) => Classified::Dependent,
        },
    )
}
