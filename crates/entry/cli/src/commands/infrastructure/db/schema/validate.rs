use std::collections::HashSet;

use anyhow::{Context, Result};
use systemprompt_database::DatabaseAdminService;
use systemprompt_logging::CliService;

use crate::cli_settings::CliConfig;
use crate::commands::infrastructure::db::types::DbValidateOutput;
use crate::shared::{CommandOutput, render_result};

pub(in crate::commands::infrastructure::db) async fn execute_validate(
    admin: &DatabaseAdminService,
    config: &CliConfig,
) -> Result<()> {
    let info = admin
        .get_database_info()
        .await
        .context("Failed to get database info")?;

    let expected_tables: Vec<&str> = DatabaseAdminService::list_expected_tables();
    let table_names: Vec<String> = info.tables.iter().map(|t| t.name.clone()).collect();
    let actual_tables: HashSet<&str> = table_names.iter().map(String::as_str).collect();

    let missing: Vec<String> = expected_tables
        .iter()
        .filter(|t| !actual_tables.contains(*t))
        .map(ToString::to_string)
        .collect();

    let extra: Vec<String> = table_names
        .iter()
        .filter(|t| {
            !expected_tables.contains(&t.as_str())
                && !t.starts_with("_sqlx")
                && !t.starts_with("v_")
        })
        .cloned()
        .collect();

    let valid = missing.is_empty();

    let output = DbValidateOutput {
        valid,
        expected_tables: expected_tables.len(),
        actual_tables: table_names.len(),
        missing_tables: missing.clone(),
        extra_tables: extra.clone(),
        message: if valid {
            "Database schema is valid".to_owned()
        } else {
            format!("Database schema has {} missing table(s)", missing.len())
        },
    };

    if config.is_json_output() {
        let result = CommandOutput::card_value("Schema Validation", &output);
        render_result(&result, config);
    } else {
        CliService::section("Schema Validation");

        if valid {
            CliService::success(&output.message);
        } else {
            CliService::error(&output.message);
            CliService::info("Missing tables:");
            for table in &missing {
                CliService::info(&format!("  - {}", table));
            }
        }

        if !extra.is_empty() && config.should_show_verbose() {
            CliService::info("Extra tables (not in expected list):");
            for table in &extra {
                CliService::info(&format!("  - {}", table));
            }
        }

        CliService::info(&format!(
            "Declared by extensions: {}",
            output.expected_tables
        ));
    }

    Ok(())
}
