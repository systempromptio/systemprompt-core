use anyhow::{anyhow, Result};
use systemprompt_core_database::{DatabaseCliDisplay, QueryExecutor, QueryResult};
use systemprompt_core_logging::CliService;

use crate::cli_settings::{CliConfig, OutputFormat};

use super::helpers::{extract_relation_name, JsonError};
use super::types::DbExecuteOutput;

fn get_output_format(format_arg: &Option<String>, config: &CliConfig) -> OutputFormat {
    match format_arg.as_deref() {
        Some("json") => OutputFormat::Json,
        Some("yaml") => OutputFormat::Yaml,
        _ => config.output_format,
    }
}

fn print_query_result(result: &QueryResult, output_format: OutputFormat) {
    match output_format {
        OutputFormat::Json => CliService::json(result),
        OutputFormat::Yaml => CliService::yaml(result),
        OutputFormat::Table => result.display_with_cli(),
    }
}

pub async fn execute_query(
    executor: &QueryExecutor,
    sql: &str,
    limit: Option<u32>,
    offset: Option<u32>,
    format: &Option<String>,
    config: &CliConfig,
) -> Result<()> {
    let output_format = get_output_format(format, config);

    let final_sql = if limit.is_some() || offset.is_some() {
        let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
        let offset_clause = offset.map(|o| format!(" OFFSET {}", o)).unwrap_or_default();
        format!("{}{}{}", sql.trim_end_matches(';'), limit_clause, offset_clause)
    } else {
        sql.to_string()
    };

    let result = executor
        .execute_query(&final_sql, true)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("does not exist") {
                let json_err = JsonError::table_not_found(&extract_relation_name(&msg));
                if config.is_json_output() {
                    CliService::json(&json_err);
                }
                anyhow!("{}", json_err.message)
            } else if msg.contains("syntax error") {
                anyhow!("SQL syntax error: {}", msg)
            } else {
                anyhow!("Query failed: {}", msg)
            }
        })?;

    if config.should_show_verbose() {
        CliService::verbose(&format!(
            "Query returned {} rows in {}ms",
            result.row_count, result.execution_time_ms
        ));
    }

    print_query_result(&result, output_format);
    Ok(())
}

pub async fn execute_write(
    executor: &QueryExecutor,
    sql: &str,
    format: &Option<String>,
    config: &CliConfig,
) -> Result<()> {
    let output_format = get_output_format(format, config);

    let result = executor.execute_query(sql, false).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("does not exist") {
            anyhow!("Table or column not found: {}", extract_relation_name(&msg))
        } else if msg.contains("syntax error") {
            anyhow!("SQL syntax error: {}", msg)
        } else if msg.contains("violates") {
            anyhow!("Constraint violation: {}", msg)
        } else {
            anyhow!("Execution failed: {}", msg)
        }
    })?;

    let output = DbExecuteOutput {
        rows_affected: result.row_count as u64,
        execution_time_ms: result.execution_time_ms,
        message: format!(
            "Query executed successfully, {} row(s) affected",
            result.row_count
        ),
    };

    if matches!(output_format, OutputFormat::Json) {
        CliService::json(&output);
    } else if matches!(output_format, OutputFormat::Yaml) {
        CliService::yaml(&output);
    } else {
        CliService::success(&output.message);
        if config.should_show_verbose() {
            CliService::verbose(&format!(
                "Execution completed in {}ms",
                result.execution_time_ms
            ));
        }
    }

    Ok(())
}
