//! Execution matrices are exposed through bounded owner-scoped cursor pages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::collections::{Cursor, Page};
use super::optimization_error::OptimizationHttpError;
use axum::Json;
use axum::extract::{Path, Query, State};
use systemprompt_evaluation::experiments::records::{ExecutionRecord, ExperimentRecord};
use systemprompt_identifiers::EvalExperimentId;
use systemprompt_runtime::AppContext;
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct ExperimentPage {
    pub experiment: ExperimentRecord,
    pub executions: Page<ExecutionRecord>,
}
pub(super) async fn detail(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalExperimentId>,
) -> Result<Json<ExperimentPage>, OptimizationHttpError> {
    let experiment = ctx
        .evaluation_repositories()
        .experiments
        .record(ctx.system_admin().id(), &id)
        .await?;
    let items = ctx
        .evaluation_repositories()
        .experiments
        .execution_page(ctx.system_admin().id(), &id, None, 50)
        .await?;
    let next_cursor = if items.len() == 50 {
        items.last().map(|item| item.id.to_string())
    } else {
        None
    };
    Ok(Json(ExperimentPage {
        experiment,
        executions: Page { items, next_cursor },
    }))
}
pub(super) async fn list(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalExperimentId>,
    Query(query): Query<Cursor>,
) -> Result<Json<Page<ExecutionRecord>>, OptimizationHttpError> {
    let limit = query.limit()?;
    let items = ctx
        .evaluation_repositories()
        .experiments
        .execution_page(ctx.system_admin().id(), &id, query.after.as_deref(), limit)
        .await?;
    let next_cursor = if items.len() == limit as usize {
        items.last().map(|item| item.id.to_string())
    } else {
        None
    };
    Ok(Json(Page { items, next_cursor }))
}
