//! Unit tests for systemprompt-scheduler crate
//!
//! Tests cover:
//! - JobStatus enum serialization and display
//! - SchedulerError construction and formatting
//! - DesiredStatus and ServiceAction state types
//! - VerifiedServiceState action determination logic
//! - ReconciliationResult tracking and success conditions
//! - StartupPlan / RestartPlan pure plan computation
//! - JobExecutionService parameter parsing, selection, and run recording

#![allow(clippy::all)]

#[cfg(test)]
mod bootstrap;
#[cfg(test)]
mod bootstrap_dispatch_db;
#[cfg(test)]
mod error_variants;
#[cfg(test)]
mod extended_jobs;
#[cfg(test)]
mod job_config;
#[cfg(test)]
mod job_execution_db;
#[cfg(test)]
mod jobs;
#[cfg(test)]
mod jobs_db;
#[cfg(test)]
mod jobs_seeded_db;
#[cfg(test)]
mod models;
#[cfg(test)]
mod orchestration;
#[cfg(test)]
mod plans;
#[cfg(test)]
mod posix_backend;
#[cfg(test)]
mod process_cleanup;
#[cfg(test)]
mod reconciler_db;
#[cfg(test)]
mod repository_db;
#[cfg(test)]
mod service_config_manifest;
#[cfg(test)]
mod service_management_behaviour_db;
#[cfg(test)]
mod service_management_db;
#[cfg(test)]
mod state_transitions;
#[cfg(test)]
mod state_verifier_seeded_db;
#[cfg(test)]
mod unit_tests;
