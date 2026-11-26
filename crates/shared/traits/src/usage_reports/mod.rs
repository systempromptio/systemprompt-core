//! Report ports over the report views a member owns.
//!
//! Each report family reads views that the member owning the underlying
//! tables declares: AI usage and cost (`ai`), agent activity and
//! conversations (`agent`), tool usage (`mcp`) and content audience
//! (`content`). The owning member implements its port over its own
//! repository and provides it; the analytics commands read it with
//! `optional`, so a composition without the owner answers
//! [`ReportUnavailable`] instead of querying a relation it never installed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent;
pub mod ai;
pub mod content;
pub mod mcp;

/// A report the composition cannot answer: the member owning its views is
/// not admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{report} is not available in this build: the {capability} capability is not admitted")]
pub struct ReportUnavailable {
    pub report: &'static str,
    pub capability: &'static str,
}
