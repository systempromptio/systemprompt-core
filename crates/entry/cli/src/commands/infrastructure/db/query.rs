//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use systemprompt_database::QueryExecutor;

use crate::CliConfig;
use crate::shared::CommandOutput;

use super::helpers::{extract_relation_name, suggest_table_name};
use super::types::DbExecuteOutput;

pub(super) struct QueryParams<'a> {
    pub sql: &'a str,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub(super) async fn execute_query(
    executor: &QueryExecutor,
    params: &QueryParams<'_>,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let final_sql = match (params.limit, params.offset) {
        (None, None) => params.sql.to_owned(),
        (limit, offset) => {
            let mut sql = params.sql.trim_end_matches(';').to_owned();
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
        .execute_readonly(&final_sql, None)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("does not exist") {
                let table_name = extract_relation_name(&msg);
                suggest_table_name(&table_name).map_or_else(
                    || anyhow!("{}", msg),
                    |suggestion| anyhow!("{}\nHint: Did you mean '{}'?", msg, suggestion),
                )
            } else {
                anyhow!("{}", msg)
            }
        })?;

    let columns = result.columns.clone();

    Ok(CommandOutput::table_of(columns, &result.rows).with_title("Query Results"))
}

pub(super) async fn execute_write(
    executor: &QueryExecutor,
    sql: &str,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let result = executor
        .execute_write(sql)
        .await
        .map_err(|e| anyhow!("{}", e))?;

    let output = DbExecuteOutput {
        rows_affected: result.row_count as u64,
        execution_time_ms: result.execution_time_ms,
        message: format!(
            "Query executed successfully, {} row(s) affected",
            result.row_count
        ),
    };

    Ok(CommandOutput::card_value("Query Executed", &output))
}
