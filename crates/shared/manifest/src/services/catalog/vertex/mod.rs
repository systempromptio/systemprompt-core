//! Vertex AI: the embedded rate card that prices and allowlists Vertex
//! models, and the report of what boot-time discovery did with it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod discovery_report;
mod error;
mod rate_card;

pub use discovery_report::DiscoveryReport;
pub use error::{VertexRateCardDefect, VertexRateCardError};
pub use rate_card::{
    DocumentedLaunchStage, RETIREMENT_NOTICE_DAYS, VertexRateCard, VertexRateCardEntry,
};
