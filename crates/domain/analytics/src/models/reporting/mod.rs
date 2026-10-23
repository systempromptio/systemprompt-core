//! Row DTOs backing analytics reports. Each submodule groups rows for a
//! single report family (agents, content, overview, requests, sessions,
//! tools); `systemprompt-cli` is the primary consumer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod agent;
mod content;
mod overview;
mod request;
mod session;
mod tool;

pub use agent::{
    AgentAiStatsRow, AgentErrorRow, AgentHourlyRow, AgentListRow, AgentStatsRow,
    AgentStatusBreakdownRow, AgentSummaryRow, AgentTaskRow, ConversationListRow,
    GatewaySessionListRow, TimestampRow,
};
pub use content::{
    BotTotalsRow, BotTypeRow, ContentStatsRow, ContentTrendRow, DeviceRow, GeoRow, TopContentRow,
    TrafficNavigationRow, TrafficPageRow, TrafficSourceRow,
};
pub use overview::{OverviewAgentRow, OverviewCostRow, OverviewRequestRow, OverviewToolRow};
pub use request::{
    ContextGroupRow, ContextSummaryRow, CostBreakdownRow, CostSummaryRow, CostTrendRow,
    CostUserBreakdownRow, ModelUsageRow, PreviousCostRow, RecentContextRow, RequestListFilter,
    RequestListRow, RequestStatsRow, RequestTrendRow,
};
pub use session::{LiveSessionRow, SessionStatsRow, SessionTrendRow};
pub use tool::{
    ToolAgentUsageRow, ToolCaller, ToolErrorRow, ToolExecutionRow, ToolListRow, ToolStatsRow,
    ToolStatusBreakdownRow, ToolSummaryRow,
};
