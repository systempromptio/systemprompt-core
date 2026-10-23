//! Public DTO/row types for the analytics crate.
//!
//! Submodules are split by domain (engagement, events, fingerprint)
//! plus a `reporting` family of row structs consumed by report frontends such
//! as `systemprompt-cli`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod engagement;
mod events;
mod fingerprint;
pub mod reporting;
mod rows;

pub use engagement::{CreateEngagementEventInput, EngagementEvent, EngagementOptionalMetrics};
pub use events::{
    AnalyticsEventBatchResponse, AnalyticsEventCreated, AnalyticsEventType, ConversionEventData,
    CreateAnalyticsEventBatchInput, CreateAnalyticsEventInput, EngagementEventData,
    LinkClickEventData, ScrollEventData,
};
pub use fingerprint::{FingerprintAnalysisResult, FingerprintReputation, FlagReason};
pub use reporting::{
    AgentAiStatsRow, AgentErrorRow, AgentHourlyRow, AgentListRow, AgentStatsRow,
    AgentStatusBreakdownRow, AgentSummaryRow, AgentTaskRow, BotTotalsRow, BotTypeRow,
    ContentStatsRow, ContentTrendRow, ContextGroupRow, ContextSummaryRow, ConversationListRow,
    CostBreakdownRow, CostSummaryRow, CostTrendRow, CostUserBreakdownRow, DeviceRow,
    GatewaySessionListRow, GeoRow, LiveSessionRow, ModelUsageRow, OverviewAgentRow,
    OverviewCostRow, OverviewRequestRow, OverviewToolRow, PreviousCostRow, RecentContextRow,
    RequestListFilter, RequestListRow, RequestStatsRow, RequestTrendRow, SessionStatsRow,
    SessionTrendRow, TimestampRow, ToolAgentUsageRow, ToolCaller, ToolErrorRow, ToolExecutionRow,
    ToolListRow, ToolStatsRow, ToolStatusBreakdownRow, ToolSummaryRow, TopContentRow,
    TrafficNavigationRow, TrafficPageRow, TrafficSourceRow,
};
pub(crate) use rows::{AgentListDbRow, RecentContextDbRow, ToolAgentUsageDbRow, ToolListDbRow};
