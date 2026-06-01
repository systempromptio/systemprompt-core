//! Unit tests for systemprompt-generator crate
//!
//! Tests cover:
//! - BuildMode parsing, display, equality, clone/copy
//! - BuildError variants, display, debug, and From conversions
//! - Markdown rendering (headings, formatting, code, lists, tables, links,
//!   images)
//! - YAML frontmatter extraction and parsing
//! - Sitemap XML generation with URL escaping
//! - Sitemap index building for chunked sitemaps
//! - RSS feed XML generation with proper date formatting
//! - RSS channel and item structures
//! - XML escaping for special characters

#![allow(clippy::all)]

#[cfg(test)]
mod asset_tests;
#[cfg(test)]
pub(crate) mod build;
#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod content;
#[cfg(test)]
mod copy_assets_tests;
#[cfg(test)]
mod css_steps_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod markdown_extra_tests;
#[cfg(test)]
mod markdown_tests;
#[cfg(test)]
mod orchestrator_validate;
#[cfg(test)]
mod pipeline_full;
#[cfg(test)]
mod pipeline_smoke;
#[cfg(test)]
mod rss_extra_tests;
#[cfg(test)]
mod sitemap;
#[cfg(test)]
mod sitemap_alternates_tests;
#[cfg(test)]
mod sitemap_provider_tests;
#[cfg(test)]
mod sitemap_tests;
#[cfg(test)]
pub(crate) mod templates;
#[cfg(test)]
mod types_tests;
