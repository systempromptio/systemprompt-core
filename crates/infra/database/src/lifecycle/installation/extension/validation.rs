//! Post-install verification of extension-declared required columns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{Extension, LoaderError};
use systemprompt_identifiers::ExtensionId;

use super::super::prepare::{ColumnsToValidate, PreparedSchema};
use crate::services::DatabaseProvider;

pub(super) fn validate_table_ownership(
    prepared: &[PreparedSchema],
    schema_extensions: &[std::sync::Arc<dyn Extension>],
) -> Result<(), LoaderError> {
    let mut owners: std::collections::HashMap<&str, &ExtensionId> =
        std::collections::HashMap::new();
    for p in prepared {
        for table in &p.owned_tables {
            if let Some(prev) = owners.insert(table.as_str(), &p.extension_id)
                && *prev != p.extension_id
            {
                return Err(LoaderError::DuplicateTableOwner {
                    table: table.clone(),
                    extension_a: prev.clone(),
                    extension_b: p.extension_id.clone(),
                });
            }
        }
    }

    for ext in schema_extensions {
        let ext_id = ext.id();
        for table in ext.cross_extension_tables() {
            let owned_elsewhere = owners.get(table).is_some_and(|&owner| owner != ext_id);
            if !owned_elsewhere {
                return Err(LoaderError::CrossExtensionTableNotOwned {
                    extension: ExtensionId::new(ext_id),
                    table: table.to_owned(),
                });
            }
        }
    }

    Ok(())
}

pub(super) async fn validate_extension_columns(
    db: &dyn DatabaseProvider,
    cols: &ColumnsToValidate,
    extension_id: &ExtensionId,
) -> Result<(), LoaderError> {
    for column in &cols.columns {
        validate_single_column(db, &cols.schema, &cols.table, column, extension_id).await?;
    }
    Ok(())
}

async fn validate_single_column(
    db: &dyn DatabaseProvider,
    schema: &str,
    table: &str,
    column: &str,
    extension_id: &ExtensionId,
) -> Result<(), LoaderError> {
    let result = db
        .query_raw_with(
            &"SELECT 1 FROM information_schema.columns WHERE table_schema = $1 AND table_name = \
              $2 AND column_name = $3",
            &[&schema, &table, &column],
        )
        .await
        .map_err(|e| LoaderError::SchemaInstallationStepFailed {
            extension: extension_id.clone(),
            context: format!("Failed to validate column '{column}'"),
            source: Box::new(e),
        })?;

    if result.rows.is_empty() {
        return Err(LoaderError::SchemaInstallationFailed {
            extension: extension_id.clone(),
            message: format!("Required column '{column}' not found in table '{schema}.{table}'"),
        });
    }

    Ok(())
}
