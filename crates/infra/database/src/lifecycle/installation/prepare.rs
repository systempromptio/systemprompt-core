//! Parse, lint, and phase-classify an extension's declarative schema before
//! any database I/O. The resulting [`PreparedSchema`] is executed by the
//! installer in the correct global phase. Routines (`CREATE OR REPLACE
//! FUNCTION`) are kept in a phase of their own as well as in the dependent
//! phase: the installer applies them before any migration runs, so a
//! migration can reference a function only the declarative schema defines.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{Extension, LoaderError};
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::BoxedSource;
use tracing::warn;

use super::classify::{Classified, classify_statement};
use super::fk_deferral::DeferredForeignKey;
use crate::services::SqlExecutor;
use crate::services::schema_linter::{created_table_names, lint_declarative_schemas};

pub(super) struct PreparedSchema {
    pub(super) extension_id: ExtensionId,
    pub(super) structural: Vec<String>,
    pub(super) routines: Vec<String>,
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
    let extension_id = ExtensionId::new(ext.metadata().id);

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
    let parse_failed = |source: BoxedSource| LoaderError::SchemaInstallationStepFailed {
        extension: extension_id.clone(),
        context: "SQL parse failed".to_owned(),
        source,
    };
    let owned_tables = created_table_names(&combined).map_err(|e| parse_failed(Box::new(e)))?;
    let parsed =
        SqlExecutor::parse_sql_statements(&combined).map_err(|e| parse_failed(Box::new(e)))?;

    let Phased {
        structural,
        routines,
        dependent,
        foreign_keys,
    } = phase_statements(&extension_id, parsed)?;

    Ok(PreparedSchema {
        extension_id,
        structural,
        routines,
        dependent,
        foreign_keys,
        columns_to_validate,
        owned_tables,
    })
}

// Why: one lint call over every file, so a foreign key in one file is
// checked against the table another file of the same extension declares.
fn lint_schemas(
    extension_id: &ExtensionId,
    schemas: &[systemprompt_extension::SchemaDefinition],
) -> Vec<String> {
    let lint_inputs: Vec<(&str, &str)> = schemas
        .iter()
        .map(|schema| {
            (
                schema.table.as_deref().unwrap_or(extension_id.as_str()),
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
    routines: Vec<String>,
    dependent: Vec<String>,
    foreign_keys: Vec<DeferredForeignKey>,
}

fn phase_statements(
    extension_id: &ExtensionId,
    parsed: Vec<String>,
) -> Result<Phased, LoaderError> {
    let mut phased = Phased {
        structural: Vec::new(),
        routines: Vec::new(),
        dependent: Vec::new(),
        foreign_keys: Vec::new(),
    };
    for statement in parsed {
        let classified = classify_statement(&statement).map_err(|e| {
            LoaderError::SchemaInstallationStepFailed {
                extension: extension_id.clone(),
                context: "declarative statement rejected".to_owned(),
                source: Box::new(e),
            }
        })?;
        match classified {
            Classified::Structural => phased.structural.push(statement),
            // Why: applied twice — before migrations with function bodies
            // unchecked, and again in the dependent phase where the final
            // body is validated against the migrated schema.
            Classified::Routine => {
                phased.routines.push(statement.clone());
                phased.dependent.push(statement);
            },
            Classified::Dependent => phased.dependent.push(statement),
            Classified::CreateTable(split) => {
                phased.structural.push(split.create_table_sql);
                phased.foreign_keys.extend(split.foreign_keys);
            },
        }
    }
    Ok(phased)
}

fn require_declarative_schema(
    extension_id: &ExtensionId,
    lint_errors: &[String],
) -> Result<(), LoaderError> {
    if lint_errors.is_empty() {
        return Ok(());
    }
    Err(LoaderError::SchemaInstallationFailed {
        extension: extension_id.clone(),
        message: format!(
            "Imperative SQL detected in declarative schema. Move offending statements to \
             schema/migrations/NNN_<name>.sql and declare them via \
             Extension::migrations():\n{}",
            lint_errors.join("\n")
        ),
    })
}
