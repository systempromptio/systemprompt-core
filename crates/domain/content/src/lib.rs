//! # systemprompt-content
//!
//! Markdown content management, ingestion, search, and link analytics for the
//! systemprompt.io AI governance dashboards. The crate provides:
//!
//! - **Models** — typed [`Content`], [`ContentMetadata`], [`IngestionOptions`],
//!   [`IngestionReport`], [`SearchRequest`], and link-tracking types.
//! - **Repositories** — [`ContentRepository`], [`SearchRepository`],
//!   [`LinkAnalyticsRepository`] backed by Postgres + `sqlx` macros.
//! - **Services** — [`IngestionService`], [`SearchService`],
//!   [`LinkGenerationService`], [`LinkAnalyticsService`], plus a
//!   [`DefaultContentProvider`] for downstream consumers.
//! - **Default providers** — [`DefaultBrandingProvider`],
//!   [`DefaultHomepagePrerenderer`], and [`DefaultListBrandingProvider`] for
//!   site-generation scaffolding.
//! - **Job entrypoint** — [`execute_content_ingestion`] runs a single ingestion
//!   pass against the active profile.
//!
//! ## Feature flags
//!
//! | Feature | Default | Effect |
//! |---------|---------|--------|
//! | _none_  | n/a     | The crate exposes a single feature surface; all modules are compiled unconditionally. The `[package.metadata.docs.rs] all-features = true` setting is retained so future feature additions automatically appear in published docs. |
//!
//! ## Layering
//!
//! `systemprompt-content` is a **domain** crate. It depends downward on
//! `systemprompt-database`, `systemprompt-cloud`, `systemprompt-extension`,
//! `systemprompt-models`, `systemprompt-traits`,
//! `systemprompt-provider-contracts`, `systemprompt-logging`, and
//! `systemprompt-identifiers`.

#![expect(
    clippy::use_self,
    reason = "spelling the concrete type clarifies intent in branding/config builders where Self would be ambiguous across nested impl blocks"
)]

pub(crate) mod branding_provider;
pub(crate) mod config;
pub mod error;
pub(crate) mod extension;
pub(crate) mod homepage_prerenderer;
pub(crate) mod jobs;
pub(crate) mod list_branding_provider;
pub(crate) mod list_items_renderer;
pub mod models;
pub mod repository;
pub mod services;

pub use branding_provider::{DefaultBrandingProvider, default_branding_provider};
pub use extension::ContentExtension;
pub use homepage_prerenderer::{DefaultHomepagePrerenderer, default_homepage_prerenderer};
pub use list_branding_provider::{DefaultListBrandingProvider, default_list_branding_provider};
pub use list_items_renderer::{ListItemsCardRenderer, default_list_items_renderer};

pub use config::{
    ContentConfigValidated, ContentReady, ContentSourceConfigValidated, LoadStats, ParsedContent,
    ValidationResult,
};
pub use error::{ContentError, ContentResult};
pub use services::validate_content_metadata;

pub use models::{
    CategoryIdUpdate, Content, ContentMetadata, IngestionOptions, IngestionReport, IngestionSource,
    SearchFilters, SearchRequest, SearchResponse, SearchResult, UpdateContentParams,
};

pub use repository::{ContentRepository, LinkAnalyticsRepository, SearchRepository};

pub use services::{
    DefaultContentProvider, GenerateLinkParams, IngestionService, LinkAnalyticsService,
    LinkGenerationService, SearchService,
};

pub use models::{LinkType, TrackClickParams, UtmParams};

pub use jobs::execute_content_ingestion;
