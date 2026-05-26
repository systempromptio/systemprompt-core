//! Integration tests for the A2A task state machine, concurrent transitions,
//! notification-status validation, and message-ID dedup.
//!
//! These tests require a running PostgreSQL with the systemprompt schema.
//! Set `DATABASE_URL` before running.

#[cfg(test)]
mod common;

#[cfg(test)]
mod state_machine_tests;

#[cfg(test)]
mod concurrent_transitions_tests;

#[cfg(test)]
mod notification_status_tests;

#[cfg(test)]
mod message_dedup_tests;

#[cfg(test)]
mod repositories_e2e;

#[cfg(test)]
mod process_utilities_tests;

#[cfg(test)]
mod services_e2e;

#[cfg(test)]
mod webhook_service_tests;

#[cfg(test)]
mod task_builder_tests;
