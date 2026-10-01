//! `infra db query` command executing ad-hoc SQL.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use systemprompt_database::{DatabaseAdminService, QueryExecutor};

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
    admin_service: &DatabaseAdminService,
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

    let result = match executor.execute_readonly(&final_sql, None).await {
        Ok(result) => result,
        Err(e) => return Err(explain_query_error(&e.to_string(), admin_service).await),
    };

    let columns = result.columns.clone();

    Ok(CommandOutput::table_of(columns, &result.rows).with_title("Query Results"))
}

async fn explain_query_error(msg: &str, admin_service: &DatabaseAdminService) -> anyhow::Error {
    if !msg.contains("does not exist") {
        return anyhow!("{}", msg);
    }
    let tables: Vec<String> = match admin_service.list_tables().await {
        Ok(tables) => tables.into_iter().map(|table| table.name).collect(),
        Err(e) => return anyhow!("{msg}\n(table suggestions unavailable: {e})"),
    };
    let table_name = extract_relation_name(msg);
    suggest_table_name(&table_name, &tables).map_or_else(
        || anyhow!("{}", msg),
        |suggestion| anyhow!("{}\nHint: Did you mean '{}'?", msg, suggestion),
    )
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
