use systemprompt_extension::{Extension, LoaderError};

use super::super::prepare::{ColumnsToValidate, PreparedSchema};
use crate::services::DatabaseProvider;

/// Ownership is derived from each extension's parsed `CREATE TABLE`
/// statements, never declared — this is the boot-time guard against two
/// extensions silently diverging on a table both create.
pub(super) fn validate_table_ownership(
    prepared: &[PreparedSchema],
    schema_extensions: &[std::sync::Arc<dyn Extension>],
) -> Result<(), LoaderError> {
    let mut owners: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for p in prepared {
        for table in &p.owned_tables {
            if let Some(prev) = owners.insert(table.as_str(), p.extension_id.as_str()) {
                if prev != p.extension_id {
                    return Err(LoaderError::DuplicateTableOwner {
                        table: table.clone(),
                        extension_a: prev.to_string(),
                        extension_b: p.extension_id.clone(),
                    });
                }
            }
        }
    }

    for ext in schema_extensions {
        let ext_id = ext.id();
        for table in ext.cross_extension_tables() {
            let owned_elsewhere = owners.get(table).is_some_and(|&owner| owner != ext_id);
            if !owned_elsewhere {
                return Err(LoaderError::CrossExtensionTableNotOwned {
                    extension: ext_id.to_string(),
                    table: table.to_string(),
                });
            }
        }
    }

    Ok(())
}

pub(super) async fn validate_extension_columns(
    db: &dyn DatabaseProvider,
    cols: &ColumnsToValidate,
    extension_id: &str,
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
    extension_id: &str,
) -> Result<(), LoaderError> {
    let result = db
        .query_raw_with(
            &"SELECT 1 FROM information_schema.columns WHERE table_schema = $1 AND table_name = \
              $2 AND column_name = $3",
            &[&schema, &table, &column],
        )
        .await
        .map_err(|e| LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Failed to validate column '{column}': {e}"),
        })?;

    if result.rows.is_empty() {
        return Err(LoaderError::SchemaInstallationFailed {
            extension: extension_id.to_string(),
            message: format!("Required column '{column}' not found in table '{schema}.{table}'"),
        });
    }

    Ok(())
}
