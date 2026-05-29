//! Unit tests for systemprompt-template-provider crate
//!
//! Tests cover:
//! - TemplateLoaderError error variants and formatting
//! - EmbeddedLoader behavior
//! - FileSystemLoader with tokio feature

#![allow(clippy::all)]

#[cfg(test)]
mod components;
#[cfg(test)]
mod error;
#[cfg(test)]
mod loader;
#[cfg(test)]
mod template_definition;
