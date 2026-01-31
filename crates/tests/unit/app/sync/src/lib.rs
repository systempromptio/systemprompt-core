//! Unit tests for systemprompt-sync crate
//!
//! Tests cover:
//! - SyncDirection enum serialization
//! - SyncConfig and SyncConfigBuilder
//! - SyncOperationResult construction
//! - SyncError types and retryable logic
//! - Diff models (ContentDiffResult, SkillsDiffResult, PlaybooksDiffResult)
//! - File types (FileBundle, FileManifest, FileEntry)
//! - Hash computation functions

#![allow(clippy::all)]

mod config;
mod error;
mod files;
mod models;
