//! Unit tests for systemprompt-sync crate
//!
//! Tests cover:
//! - SyncDirection enum serialization
//! - SyncConfig and SyncConfigBuilder
//! - SyncOperationResult construction
//! - SyncError types and retryable logic
//! - Diff models (ContentDiffResult, SkillsDiffResult)
//! - File types (FileBundle, FileManifest, FileEntry)
//! - Hash computation functions

#![allow(clippy::all)]

#[cfg(test)]
mod config;
#[cfg(test)]
mod error;
#[cfg(test)]
mod files;
#[cfg(test)]
mod models;
