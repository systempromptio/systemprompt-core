//! Unit tests for systemprompt-generator crate
//!
//! Tests cover:
//! - BuildMode parsing, display, equality, clone/copy
//! - BuildError variants, display, debug, and From conversions
//! - Markdown rendering (headings, formatting, code, lists, tables, links, images)
//! - YAML frontmatter extraction and parsing
//! - Content card generation (content cards, related cards, image HTML)
//! - Image URL normalization and absolute URL generation
//! - Sitemap XML generation with URL escaping
//! - Sitemap index building for chunked sitemaps
//! - RSS feed XML generation with proper date formatting
//! - RSS channel and item structures
//! - XML escaping for special characters
//! - Footer HTML generation from YAML config
//! - Navigation and social link generation

#![allow(clippy::all)]

mod build;
mod content;
mod sitemap;
mod templates;
