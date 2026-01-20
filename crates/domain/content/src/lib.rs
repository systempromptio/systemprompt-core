#![allow(clippy::use_self)]

pub mod analytics;
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
pub use services::{
    validate_content_metadata, validate_paper_metadata, validate_paper_section_ids_unique,
};

pub use models::{
    Content, ContentMetadata, IngestionOptions, IngestionReport, IngestionSource, SearchFilters,
    SearchRequest, SearchResponse, SearchResult, UpdateContentParams,
};

pub use repository::{ContentRepository, SearchRepository};

pub use services::{DefaultContentProvider, IngestionService, SearchService};

pub use api::{get_content_handler, list_content_by_source_handler, query_handler, router};

pub use analytics::{LinkAnalyticsRepository, LinkAnalyticsService};

pub use jobs::ContentIngestionJob;
