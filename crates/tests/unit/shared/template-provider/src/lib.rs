//! Unit tests for systemprompt-template-provider crate
//!
//! Tests cover:
//! - TemplateLoaderError error variants and formatting
//! - EmbeddedLoader behavior
//! - FileSystemLoader with tokio feature

#![allow(clippy::all)]

mod error;
mod loader;
