//! Per-execution accounting joined from the reservations this domain holds
//! and the usage the AI request trace recorded for them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::{AiRequestId, EvalExecutionId, UserId};

use super::{EvaluationLifecycleRepository, ExecutionAccounting};
use crate::Result;
use crate::experiments::{invalid, missing};
use crate::models::AccountingStatus;

impl EvaluationLifecycleRepository {
    pub async fn execution_accounting(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
    ) -> Result<ExecutionAccounting> {
        let reservations = sqlx::query!(r#"SELECT m.request_id,r.actual FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id LEFT JOIN eval_request_reservations m ON m.execution_id=x.id LEFT JOIN eval_budget_reservations r ON r.id=m.reservation_id WHERE e.owner_id=$1 AND x.id=$2"#,
            owner.as_str(), execution.as_str()).fetch_all(&self.pool).await?;
        if reservations.is_empty() {
            return Err(missing("Execution accounting unavailable in this scope"));
        }
        let settled: BTreeMap<String, bool> = reservations
            .iter()
            .filter_map(|row| {
                row.request_id
                    .clone()
                    .map(|request| (request, row.actual.is_some()))
            })
            .collect();
        let request_ids: Vec<AiRequestId> = settled.keys().cloned().map(AiRequestId::new).collect();
        let usage = self.trace.list_usage(owner, &request_ids).await?;
        let requests = request_ids.len();
        let mut complete = 0usize;
        let mut input: u64 = 0;
        let mut output: u64 = 0;
        let mut tool_calls: u64 = 0;
        let mut cost: i64 = 0;
        for record in &usage {
            let reservation_settled = settled
                .get(record.request_id.as_str())
                .copied()
                .unwrap_or(false);
            if record.is_settled()
                && record.input_tokens.is_some()
                && record.output_tokens.is_some()
                && reservation_settled
            {
                complete += 1;
            }
            input += counted("input_tokens", i64::from(record.input_tokens.unwrap_or(0)))?;
            output += counted(
                "output_tokens",
                i64::from(record.output_tokens.unwrap_or(0)),
            )?;
            tool_calls += record.tool_calls;
            cost += record.cost_microdollars;
        }
        let status = if requests == 0 {
            AccountingStatus::Unknown
        } else if complete == requests {
            AccountingStatus::Complete
        } else {
            AccountingStatus::Partial
        };
        Ok(ExecutionAccounting {
            input_tokens: (requests > 0).then_some(input),
            output_tokens: (requests > 0).then_some(output),
            tool_calls,
            attempted_cost_microdollars: cost,
            status,
        })
    }
}

fn counted(column: &str, value: i64) -> Result<u64> {
    u64::try_from(value)
        .map_err(|_e| invalid(&format!("Execution accounting column {column} is negative")))
}
