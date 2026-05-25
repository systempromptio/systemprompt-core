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
