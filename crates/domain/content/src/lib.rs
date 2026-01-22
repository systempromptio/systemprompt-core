#![allow(clippy::use_self)]

pub mod config;
pub mod error;
pub mod extension;
pub mod jobs;
pub mod models;
pub mod repository;
pub mod services;

pub use extension::ContentExtension;

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

pub use jobs::ContentIngestionJob;
