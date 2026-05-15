//! Parse, lint, and phase-classify an extension's declarative schema before
//! any database I/O. The resulting [`PreparedSchema`] is executed by the
//! installer in the correct global phase.

use systemprompt_extension::{Extension, LoaderError};

use crate::services::SqlExecutor;
use crate::services::schema_linter::lint_declarative_schema;

/// One extension's declarative schema, parsed and split into the structural
/// statements (run in Phase 1) and the dependent statements (run in Phase 3).
pub(super) struct PreparedSchema {
    pub(super) extension_id: String,
    pub(super) structural: Vec<String>,
    pub(super) dependent: Vec<String>,
    pub(super) columns_to_validate: Vec<(String, Vec<String>)>,
}

/// Lint, parse, and classify one extension's declarative schema. Performs no
/// database I/O.
pub(super) fn prepare_extension_schema(ext: &dyn Extension) -> Result<PreparedSchema, LoaderError> {
    let schemas = ext.schemas();
    let extension_id = ext.metadata().id.to_string();

    let mut all_sql = Vec::new();
    let mut columns_to_validate: Vec<(String, Vec<String>)> = Vec::new();
    let mut lint_errors: Vec<String> = Vec::new();

    for schema in &schemas {
        if let Err(errors) = lint_declarative_schema(&schema.sql, schema.table.as_str()) {
            for err in errors {
                lint_errors.push(err.to_string());
            }
        }

        all_sql.push(schema.sql.as_str());

        if !schema.required_columns.is_empty() {
            columns_to_validate.push((schema.table.clone(), schema.required_columns.clone()));
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
    let parsed = SqlExecutor::parse_sql_statements(&combined).map_err(|e| {
        LoaderError::SchemaInstallationFailed {
            extension: extension_id.clone(),
            message: format!("SQL parse failed: {e}"),
        }
    })?;

    let mut structural = Vec::new();
    let mut dependent = Vec::new();
    for statement in parsed {
        if statement_is_structural(&statement) {
            structural.push(statement);
        } else {
            dependent.push(statement);
        }
    }

    Ok(PreparedSchema {
        extension_id,
        structural,
        dependent,
        columns_to_validate,
    })
}

/// A statement is *structural* if it only creates objects that a migration
/// may need to exist first — tables, types, and Postgres extensions. Indexes,
/// views, functions, triggers, comments, and drops are dependent: they may
/// reference a column a migration introduces, so they run after Phase 2.
fn statement_is_structural(statement: &str) -> bool {
    let Ok(parsed) = pg_query::parse(statement) else {
        return false;
    };
    let mut saw_structural = false;
    for raw in parsed.protobuf.stmts {
        let Some(node) = raw.stmt.and_then(|s| s.node) else {
            continue;
        };
        match node {
            pg_query::NodeEnum::CreateStmt(_)
            | pg_query::NodeEnum::CompositeTypeStmt(_)
            | pg_query::NodeEnum::CreateEnumStmt(_)
            | pg_query::NodeEnum::CreateExtensionStmt(_) => saw_structural = true,
            _ => return false,
        }
    }
    saw_structural
}
