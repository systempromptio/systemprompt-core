//! Row DTOs backing the analytics-owned reports.
//!
//! These are the content audience, traffic and session rows. The reports over
//! member-owned views (agents, requests, costs, tools, content catalogue) are
//! the owning members' report ports in `systemprompt_traits::usage_reports`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content;
mod session;

pub use content::{
    BotTotalsRow, BotTypeRow, ContentStatsRow, ContentTrendRow, DeviceRow, GeoRow,
    TrafficNavigationRow, TrafficPageRow, TrafficSourceRow,
};
pub use session::{LiveSessionRow, SessionStatsRow, SessionTrendRow};
