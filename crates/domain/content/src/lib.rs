#![allow(clippy::use_self)]

pub(crate) mod branding_provider;
pub(crate) mod config;
pub(crate) mod error;
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
pub use error::ContentError;
pub use services::validate_content_metadata;

pub use models::{
    Content, ContentMetadata, IngestionOptions, IngestionReport, IngestionSource, SearchFilters,
    SearchRequest, SearchResponse, SearchResult, UpdateContentParams,
};

pub use repository::{ContentRepository, LinkAnalyticsRepository, SearchRepository};

pub use services::{
    ContentService, DefaultContentProvider, GenerateLinkParams, IngestionService,
    LinkAnalyticsService, LinkGenerationService, SearchService,
};

pub use models::{LinkType, TrackClickParams, UtmParams};

pub use jobs::execute_content_ingestion;
