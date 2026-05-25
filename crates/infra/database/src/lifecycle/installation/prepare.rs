//! Parse, lint, and phase-classify an extension's declarative schema before
//! any database I/O. The resulting [`PreparedSchema`] is executed by the
//! installer in the correct global phase.

use systemprompt_extension::{Extension, LoaderError};

use crate::services::SqlExecutor;
use crate::services::schema_linter::{created_table_names, lint_declarative_schema};

pub(super) struct PreparedSchema {
    pub(super) extension_id: String,
    pub(super) structural: Vec<String>,
    pub(super) dependent: Vec<String>,
    pub(super) columns_to_validate: Vec<ColumnsToValidate>,
    /// Tables this extension creates, derived from its `CREATE TABLE`
    /// statements — the authoritative ownership set.
    pub(super) owned_tables: Vec<String>,
}

/// `(schema, table, columns)` — the `required_columns` of one
/// [`systemprompt_extension::SchemaDefinition`], qualified by its Postgres
/// schema so validation does not assume `public`.
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
    let mut lint_errors: Vec<String> = Vec::new();

    for schema in &schemas {
        if let Err(errors) = lint_declarative_schema(&schema.sql, schema.table.as_str()) {
            for err in errors {
                lint_errors.push(err.to_string());
            }
        }

        all_sql.push(schema.sql.as_str());

        if !schema.required_columns.is_empty() {
            columns_to_validate.push(ColumnsToValidate {
                schema: schema.schema_name().to_owned(),
                table: schema.table.clone(),
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
    for statement in parsed {
        let phase = classify_statement(&statement).map_err(|message| {
            LoaderError::SchemaInstallationFailed {
                extension: extension_id.clone(),
                message,
            }
        })?;
        match phase {
            StatementPhase::Structural => structural.push(statement),
            StatementPhase::Dependent => dependent.push(statement),
        }
    }

    Ok(PreparedSchema {
        extension_id,
        structural,
        dependent,
        columns_to_validate,
        owned_tables,
    })
}

/// Install phase a single declarative statement belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatementPhase {
    /// Phase 1 — creates a schema, table, type, sequence, or extension; an
    /// object a Phase 2 migration may depend on.
    Structural,
    /// Phase 3 — indexes, views, triggers, functions, grants, comments and
    /// any ALTER; may reference a migration-added column.
    Dependent,
}

/// Classify a declarative statement into its install phase.
///
/// Every `pg_query` DDL node type is matched **explicitly**: a node the
/// classifier does not recognise is a hard error rather than a silent
/// mis-phase. A new Postgres node type therefore surfaces as a visible boot
/// failure that forces an explicit phase decision — the `CREATE SCHEMA`
/// mis-phase that this replaces could not happen here.
fn classify_statement(statement: &str) -> Result<StatementPhase, String> {
    use pg_query::NodeEnum;

    let parsed = pg_query::parse(statement)
        .map_err(|e| format!("SQL parse failed: {e}\nSQL:\n{statement}"))?;

    let mut phase: Option<StatementPhase> = None;
    for raw in parsed.protobuf.stmts {
        let Some(node) = raw.stmt.and_then(|s| s.node) else {
            continue;
        };
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

    Ok(phase.unwrap_or(StatementPhase::Dependent))
}
