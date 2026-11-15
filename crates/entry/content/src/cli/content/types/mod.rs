//! Serializable output shapes for the `core content` commands.
//!
//! Aggregates and re-exports the content, link, and pipeline output types so
//! command renderers share a single stable JSON surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content;
mod links;
mod pipeline;

pub use content::{
    ContentDetailOutput, ContentListOutput, ContentStatusRow, ContentSummary, DeleteOutput,
    DeleteSourceOutput, PopularOutput, SearchOutput, SearchResultRow, StatusOutput, UpdateOutput,
    VerifyOutput,
};
pub use links::{
    CampaignAnalyticsOutput, ClickRow, ClicksOutput, GenerateLinkOutput, JourneyNode,
    JourneyOutput, LinkDeleteOutput, LinkDetailOutput, LinkListOutput, LinkPerformanceOutput,
    LinkSummary, UtmParamsOutput,
};
pub use pipeline::ExportOutput;
