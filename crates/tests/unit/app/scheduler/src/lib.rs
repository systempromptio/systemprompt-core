//! Unit tests for systemprompt-scheduler crate
//!
//! Tests cover:
//! - JobStatus enum serialization and display
//! - SchedulerError construction and formatting
//! - DesiredStatus and ServiceAction state types
//! - VerifiedServiceState action determination logic
//! - ReconciliationResult tracking and success conditions

#![allow(clippy::all)]

mod models;
mod orchestration;
