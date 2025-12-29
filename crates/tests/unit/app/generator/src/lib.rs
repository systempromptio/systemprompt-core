//! Unit tests for systemprompt-generator crate
//!
//! Tests cover:
//! - BuildMode parsing and display
//! - BuildError variants and messages
//! - Markdown rendering and frontmatter extraction
//! - Content card generation and image URL normalization
//! - Sitemap XML generation
//! - Template paper processing (read time, TOC)
//! - Navigation HTML generation

#![allow(clippy::all)]

mod build;
mod content;
mod sitemap;
mod templates;
