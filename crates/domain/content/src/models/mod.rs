//! Content domain types: content items, links, search, and their builders.
//!
//! [`Content`] and [`ContentMetadata`] model published items; the [`link`]
//! types model campaign/destination links and their performance; the [`search`]
//! types model query requests and results. Construction parameters live in
//! [`builders`]; validation failures surface as [`ContentValidationError`].

pub mod builders;
pub mod content;
pub mod content_error;
pub mod link;
pub mod search;

pub use builders::{
    CategoryIdUpdate, CreateContentParams, CreateLinkParams, RecordClickParams, TrackClickParams,
    UpdateContentParams,
};
pub use content::{
    Content, ContentKind, ContentLinkMetadata, ContentMetadata, ContentSummary, IngestionOptions,
    IngestionReport, IngestionSource, Tag,
};
pub use content_error::ContentValidationError;
pub use link::{
    CampaignLink, CampaignPerformance, ContentJourneyNode, DestinationType, LinkClick,
    LinkPerformance, LinkType, UtmParams,
};
pub use search::{SearchFilters, SearchRequest, SearchResponse, SearchResult};
