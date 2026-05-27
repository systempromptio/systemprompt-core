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

#[cfg(test)]
mod push_notification_e2e;

#[cfg(test)]
mod agent_service_repo_e2e;

#[cfg(test)]
mod execution_step_repo_e2e;

#[cfg(test)]
mod message_service_e2e;

#[cfg(test)]
mod conversation_service_e2e;

#[cfg(test)]
mod artifact_publishing_e2e;

#[cfg(test)]
mod message_repo_e2e;

#[cfg(test)]
mod agent_database_service_e2e;

#[cfg(test)]
mod agent_monitor_e2e;

#[cfg(test)]
mod task_helper_tests;
