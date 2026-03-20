//! Unit tests for systemprompt-generator crate
//!
//! Tests cover:
//! - BuildMode parsing, display, equality, clone/copy
//! - BuildError variants, display, debug, and From conversions
//! - Markdown rendering (headings, formatting, code, lists, tables, links, images)
//! - YAML frontmatter extraction and parsing
//! - Sitemap XML generation with URL escaping
//! - Sitemap index building for chunked sitemaps
//! - RSS feed XML generation with proper date formatting
//! - RSS channel and item structures
//! - XML escaping for special characters

#![allow(clippy::all)]

#[cfg(test)]
mod build;
#[cfg(test)]
mod content;
#[cfg(test)]
mod sitemap;
#[cfg(test)]
mod templates;
