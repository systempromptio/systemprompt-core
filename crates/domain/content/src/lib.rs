#![allow(clippy::use_self)]

pub mod api;
pub mod config;
pub mod error;
pub mod jobs;
pub mod models;
pub mod repository;
pub mod services;

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

pub use services::{DefaultContentProvider, IngestionService, LinkAnalyticsService, SearchService};

pub use api::{get_content_handler, list_content_by_source_handler, query_handler, router};

pub use jobs::ContentIngestionJob;
