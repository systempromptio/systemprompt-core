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
mod api_client_mock;
#[cfg(test)]
mod api_client_retry;
#[cfg(test)]
mod config;
#[cfg(test)]
mod coverage_boost;
#[cfg(test)]
mod crate_deploy;
#[cfg(test)]
mod crate_deploy_flow;
#[cfg(test)]
mod database_export;
#[cfg(test)]
mod database_sync_failure;
#[cfg(test)]
mod deploy_artifacts;
#[cfg(test)]
mod deploy_artifacts_extra;
#[cfg(test)]
mod deploy_orchestrator;
#[cfg(test)]
mod deploy_pre_sync_apply;
#[cfg(test)]
mod diff;
#[cfg(test)]
mod diff_content;
#[cfg(test)]
mod edge_cases;
#[cfg(test)]
mod error;
#[cfg(test)]
mod extract_traversal;
#[cfg(test)]
mod file_bundler_extra;
#[cfg(test)]
mod file_sync;
#[cfg(test)]
mod file_sync_service_extra;
#[cfg(test)]
mod files;
#[cfg(test)]
mod generation;
#[cfg(test)]
mod jobs_access_control_db;
#[cfg(test)]
mod jobs_access_control_flow;
#[cfg(test)]
mod jobs_content_sync;
#[cfg(test)]
mod jobs_content_sync_db;
#[cfg(test)]
mod jobs_content_sync_flow;
#[cfg(test)]
mod jobs_smoke;
#[cfg(test)]
mod lib_sync_all;
#[cfg(test)]
mod lib_sync_db;
#[cfg(test)]
mod local_content_sync;
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
