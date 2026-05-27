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
mod access_control;
#[cfg(test)]
mod api_client;
#[cfg(test)]
mod api_client_extra;
#[cfg(test)]
mod config;
#[cfg(test)]
mod crate_deploy;
#[cfg(test)]
mod database_export;
#[cfg(test)]
mod database_sync_failure;
#[cfg(test)]
mod diff;
#[cfg(test)]
mod edge_cases;
#[cfg(test)]
mod error;
#[cfg(test)]
mod file_sync;
#[cfg(test)]
mod files;
#[cfg(test)]
mod generation;
#[cfg(test)]
mod jobs_smoke;
#[cfg(test)]
mod models;
#[cfg(test)]
mod result;
#[cfg(test)]
mod retry;
#[cfg(test)]
mod sync_service_lib;
#[cfg(test)]
mod token_exchange;
#[cfg(test)]
mod api_client_mock;
#[cfg(test)]
mod file_bundler_extra;
#[cfg(test)]
mod file_sync_service_extra;
