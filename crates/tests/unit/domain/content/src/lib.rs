//! Unit tests for systemprompt-content crate
//!
//! Tests cover:
//! - Content models (Content, ContentKind, ContentMetadata, etc.)
//! - Link models (CampaignLink, LinkClick, LinkType, UtmParams, etc.)
//! - Search models (SearchRequest, SearchFilters, SearchResult, SearchResponse)
//! - Content error types
//! - Builder patterns (CreateContentParams, CreateLinkParams, etc.)
//! - Validation services (content metadata)
//! - Link generation utilities
//! - Configuration validation

#[cfg(test)]
mod models;

#[cfg(test)]
mod error;

#[cfg(test)]
mod services;

#[cfg(test)]
mod config;
