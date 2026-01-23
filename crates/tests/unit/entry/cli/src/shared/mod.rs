//! Unit tests for CLI shared utilities
//!
//! Tests cover:
//! - ProjectRoot discovery and path handling
//! - ProjectError types and messages
//! - Command result types and builders
//! - CLI parsers
//! - Profile utilities

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

mod command_result;
mod parsers;
mod profile;
mod project;
