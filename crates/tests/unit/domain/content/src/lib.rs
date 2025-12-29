//! Unit tests for systemprompt-core-content crate
//!
//! Tests cover:
//! - Content models (Content, ContentKind, ContentMetadata, etc.)
//! - Link models (CampaignLink, LinkClick, LinkType, UtmParams, etc.)
//! - Paper models (PaperSection, PaperMetadata)
//! - Search models (SearchRequest, SearchFilters, SearchResult, SearchResponse)
//! - Content error types
//! - Builder patterns (CreateContentParams, CreateLinkParams, etc.)
//! - Validation services (content metadata, paper metadata)
//! - Link generation utilities
//! - Configuration validation
//! - API types

#[cfg(test)]
mod models;

#[cfg(test)]
mod error;

#[cfg(test)]
mod services;

#[cfg(test)]
mod config;

#[cfg(test)]
mod api;
