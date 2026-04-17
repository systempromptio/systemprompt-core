//! Unit tests for systemprompt-scheduler crate
//!
//! Tests cover:
//! - JobStatus enum serialization and display
//! - SchedulerError construction and formatting
//! - DesiredStatus and ServiceAction state types
//! - VerifiedServiceState action determination logic
//! - ReconciliationResult tracking and success conditions

#![allow(clippy::all)]

#[cfg(test)]
mod jobs;
#[cfg(test)]
mod models;
#[cfg(test)]
mod orchestration;
#[cfg(test)]
mod state_transitions;
#[cfg(test)]
mod unit_tests;
