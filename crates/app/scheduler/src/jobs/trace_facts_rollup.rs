//! Job draining the `trace_facts` projection: trace ids the statement
//! triggers on `ai_requests`, `mcp_tool_executions`, `agent_tasks` and `logs`
//! (or an upgrade) queued, each rebuilt into its one `trace_facts` row. Runs
//! every ten seconds so the trace list trails the ledger by one tick.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use super::projection_drain::{ProjectionDrain, drain_projection};

const DRAIN: ProjectionDrain = ProjectionDrain {
    projection: "trace_facts",
    projector: "refresh_trace_facts",
    batch: 500,
    budget: Duration::from_secs(20),
};

#[derive(Debug, Clone, Copy)]
pub struct TraceFactsRollupJob;

#[async_trait]
impl Job for TraceFactsRollupJob {
    fn name(&self) -> &'static str {
        "trace_facts_rollup"
    }

    fn description(&self) -> &'static str {
        "Drains queued trace ids into the stored trace facts the trace list reads"
    }

    fn schedule(&self) -> &'static str {
        "*/10 * * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        drain_projection(ctx, &DRAIN).await
    }
}

systemprompt_provider_contracts::submit_job!(&TraceFactsRollupJob);
