use anyhow::{anyhow, Result};
use systemprompt_database::{QueryExecutor, QueryResult};

use crate::shared::CommandResult;
use crate::CliConfig;

use super::helpers::{extract_relation_name, suggest_table_name};
use super::types::DbExecuteOutput;

pub struct QueryParams<'a> {
    pub sql: &'a str,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn execute_query(
    executor: &QueryExecutor,
    params: &QueryParams<'_>,
    _config: &CliConfig,
) -> Result<CommandResult<QueryResult>> {
    let final_sql = match (params.limit, params.offset) {
        (None, None) => params.sql.to_string(),
        (limit, offset) => {
            let mut sql = params.sql.trim_end_matches(';').to_string();
            if let Some(l) = limit {
                sql.push_str(&format!(" LIMIT {}", l));
            }
            if let Some(o) = offset {
                sql.push_str(&format!(" OFFSET {}", o));
            }
            sql
        },
    };

    let result = executor
        .execute_query(&final_sql, true)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("does not exist") {
                let table_name = extract_relation_name(&msg);
                suggest_table_name(&table_name).map_or_else(
                    || anyhow!("Table or relation '{}' does not exist", table_name),
                    |suggestion| {
                        anyhow!(
                            "Table or relation '{}' does not exist\nHint: Did you mean '{}'?",
                            table_name,
                            suggestion
                        )
                    },
                )
            } else if msg.contains("syntax error") {
                anyhow!("SQL syntax error: {}", msg)
            } else {
                anyhow!("Query failed: {}", msg)
            }
        })?;

    let columns = result.columns.clone();

    Ok(CommandResult::table(result)
        .with_title("Query Results")
        .with_columns(columns))
}

pub async fn execute_write(
    executor: &QueryExecutor,
    sql: &str,
    _config: &CliConfig,
) -> Result<CommandResult<DbExecuteOutput>> {
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

    Ok(CommandResult::text(output).with_title("Query Executed"))
}
