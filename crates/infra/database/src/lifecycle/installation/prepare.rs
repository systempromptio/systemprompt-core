//! Parse, lint, and phase-classify an extension's declarative schema before
//! any database I/O. The resulting [`PreparedSchema`] is executed by the
//! installer in the correct global phase.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{Extension, LoaderError};

use super::fk_deferral::{DeferredForeignKey, SplitCreateTable, split_foreign_keys};
use crate::services::SqlExecutor;
use crate::services::schema_linter::{created_table_names, lint_declarative_schemas};

pub(super) struct PreparedSchema {
    pub(super) extension_id: String,
    pub(super) structural: Vec<String>,
    pub(super) dependent: Vec<String>,
    /// Foreign keys split out of the structural `CREATE TABLE`s; applied
    /// after every extension's dependent phase.
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

    // Why: one lint call over every file, so a foreign key in one file is
    // checked against the table another file of the same extension declares.
    let lint_inputs: Vec<(&str, &str)> = schemas
        .iter()
        .map(|schema| {
            (
                schema.table.as_deref().unwrap_or(extension_id.as_str()),
                schema.sql.as_str(),
            )
        })
        .collect();
    let lint_errors: Vec<String> = lint_declarative_schemas(&lint_inputs)
        .err()
        .into_iter()
        .flatten()
        .map(|err| err.to_string())
        .collect();

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

    if !lint_errors.is_empty() {
        return Err(LoaderError::SchemaInstallationFailed {
            extension: extension_id,
            message: format!(
                "Imperative SQL detected in declarative schema. Move offending statements to \
                 schema/migrations/NNN_<name>.sql and declare them via \
                 Extension::migrations():\n{}",
                lint_errors.join("\n")
            ),
        });
    }

    let combined = all_sql.join("\n");
    let owned_tables = created_table_names(&combined);
    let parsed = SqlExecutor::parse_sql_statements(&combined).map_err(|e| {
        LoaderError::SchemaInstallationFailed {
            extension: extension_id.clone(),
            message: format!("SQL parse failed: {e}"),
        }
    })?;

    let mut structural = Vec::new();
    let mut dependent = Vec::new();
    let mut foreign_keys = Vec::new();
    for statement in parsed {
        let classified = classify_statement(&statement).map_err(|message| {
            LoaderError::SchemaInstallationFailed {
                extension: extension_id.clone(),
                message,
            }
        })?;
        match classified {
            Classified::Structural => structural.push(statement),
            Classified::Dependent => dependent.push(statement),
            Classified::CreateTable(split) => {
                structural.push(split.create_table_sql);
                foreign_keys.extend(split.foreign_keys);
            },
        }
    }

    Ok(PreparedSchema {
        extension_id,
        structural,
        dependent,
        foreign_keys,
        columns_to_validate,
        owned_tables,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatementPhase {
    Structural,
    Dependent,
}

enum Classified {
    Structural,
    Dependent,
    /// A `CREATE TABLE`, with its foreign keys deferred.
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
