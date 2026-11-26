//! A detached command kernel holding the usage report ports the ai, agent,
//! mcp, content and analytics members provide, over one database: what the
//! analytics report commands resolve from a full composition.

use std::sync::Arc;

use systemprompt_agent::repository::AgentActivityReportRepository;
use systemprompt_ai::AiUsageReportRepository;
use systemprompt_analytics::ContentAudienceRepository;
use systemprompt_content::ContentAudienceReportRepository;
use systemprompt_database::DbPool;
use systemprompt_extension::prelude::Dependencies;
use systemprompt_mcp::repository::ToolUsageReportRepository;
use systemprompt_runtime::Kernel;
use systemprompt_traits::usage_reports::agent::DynAgentActivityReport;
use systemprompt_traits::usage_reports::ai::DynAiUsageReport;
use systemprompt_traits::usage_reports::content::{
    DynContentAudienceCounts, DynContentAudienceReport,
};
use systemprompt_traits::usage_reports::mcp::DynToolUsageReport;
use systemprompt_traits::BackgroundTasks;

pub fn report_ports_kernel(pool: &DbPool) -> Arc<Kernel> {
    let ai: DynAiUsageReport = Arc::new(AiUsageReportRepository::new(pool));
    let agents: DynAgentActivityReport = Arc::new(AgentActivityReportRepository::new(pool));
    let tools: DynToolUsageReport = Arc::new(ToolUsageReportRepository::new(pool));
    let audience: DynContentAudienceReport = Arc::new(ContentAudienceReportRepository::new(pool));
    let counts: DynContentAudienceCounts = Arc::new(ContentAudienceRepository::new(pool));
    let seed = Dependencies::new()
        .with(ai)
        .with(agents)
        .with(tools)
        .with(audience)
        .with(counts);
    Arc::new(Kernel::detached(BackgroundTasks::new(), seed))
}
