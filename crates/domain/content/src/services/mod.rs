//! Content domain services: ingestion, search, link generation, and validation.
//!
//! [`IngestionService`] loads and normalizes content from disk; [`SearchService`]
//! queries it; the link services mint and track campaign/destination links; and
//! [`validate_content_metadata`] enforces frontmatter rules. [`DefaultContentProvider`]
//! is the `ContentProvider` implementation other crates consume.

pub mod content_provider;
pub mod ingestion;
pub mod link;
pub mod search;
pub mod validation;

pub use content_provider::DefaultContentProvider;
pub use ingestion::IngestionService;
pub use link::{GenerateLinkParams, LinkAnalyticsService, LinkGenerationService};
pub use search::SearchService;
pub use validation::validate_content_metadata;
