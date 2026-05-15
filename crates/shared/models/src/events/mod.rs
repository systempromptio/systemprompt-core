//! Event envelopes flowing across the system bus.
//!
//! Three event families with builders: [`A2AEvent`] (agent protocol),
//! [`AnalyticsEvent`] (page views, sessions, engagement), and
//! [`SystemEvent`] (lifecycle/health). Re-exports the AG-UI event types
//! for callers that consume all event streams from one import.

mod a2a_event;
mod a2a_event_type;
mod analytics_event;
pub mod constants;
mod context_event;
pub mod payloads;
mod system_event;
mod system_event_type;

pub use a2a_event::{A2AEvent, A2AEventBuilder};
pub use a2a_event_type::A2AEventType;
pub use analytics_event::{
    AnalyticsEvent, AnalyticsEventBuilder, EngagementUpdatePayload, PageViewPayload,
    RealTimeStatsPayload, SessionEndedPayload, SessionStartedPayload,
};
pub use context_event::ContextEvent;
pub use payloads::system::ContextSummary;
pub use system_event::{SystemEvent, SystemEventBuilder};
pub use system_event_type::SystemEventType;

pub use crate::agui::{AgUiEvent, AgUiEventBuilder, AgUiEventType};
