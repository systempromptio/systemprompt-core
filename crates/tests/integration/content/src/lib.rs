//! Integration tests for the `systemprompt-content` crate — exercises the
//! DB-backed repository and ingestion paths that pure-unit tests cannot
//! reach. `DATABASE_URL` must point at a Postgres instance with the full
//! systemprompt-core schema applied (run `systemprompt-test-migrate` first).
//!
//! Tests early-skip if `DATABASE_URL` is absent so the suite passes in
//! environments without a Postgres fixture.

#[cfg(test)]
mod content_repository_tests;

#[cfg(test)]
mod ingestion_service_tests;

#[cfg(test)]
mod link_repository_tests;

#[cfg(test)]
mod search_repository_tests;

#[cfg(test)]
mod link_service_tests;
