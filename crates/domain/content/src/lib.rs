#![allow(clippy::use_self)]

pub mod branding_provider;
pub mod config;
pub mod error;
pub mod extension;
pub mod homepage_prerenderer;
pub mod jobs;
pub mod list_branding_provider;
pub mod list_items_renderer;
pub mod models;
pub mod repository;
pub mod services;

pub use branding_provider::{default_branding_provider, DefaultBrandingProvider};
pub use extension::ContentExtension;
pub use homepage_prerenderer::{default_homepage_prerenderer, DefaultHomepagePrerenderer};
pub use list_branding_provider::{default_list_branding_provider, DefaultListBrandingProvider};
pub use list_items_renderer::{default_list_items_renderer, ListItemsCardRenderer};

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
