//! Unit tests for the systemprompt-identifiers crate.
//!
//! This crate contains all tests for identifier types, following
//! the project's testing policy of keeping tests in separate crates.

#[cfg(test)]
mod actor_tests;

#[cfg(test)]
mod agent_tests;

#[cfg(test)]
mod from_value_tests;

#[cfg(test)]
mod macros_helpers_tests;

#[cfg(test)]
mod to_db_value_impls;
#[cfg(test)]
mod to_value_tests;

#[cfg(test)]
mod ai_tests;

#[cfg(test)]
mod auth_tests;

#[cfg(test)]
mod client_tests;

#[cfg(test)]
mod content_tests;

#[cfg(test)]
mod context_tests;

#[cfg(test)]
mod coverage_extra_tests;

#[cfg(test)]
mod locale_tests;

#[cfg(test)]
mod email_tests;

#[cfg(test)]
mod execution_tests;

#[cfg(test)]
mod gateway_conversation_tests;

#[cfg(test)]
mod provider_request_tests;

#[cfg(test)]
mod jobs_tests;

#[cfg(test)]
mod links_tests;

#[cfg(test)]
mod mcp_tests;

#[cfg(test)]
mod path_tests;

#[cfg(test)]
mod profile_tests;

#[cfg(test)]
mod roles_tests;

#[cfg(test)]
mod session_tests;

#[cfg(test)]
mod task_tests;

#[cfg(test)]
mod trace_tests;

#[cfg(test)]
mod url_tests;

#[cfg(test)]
mod user_tests;
