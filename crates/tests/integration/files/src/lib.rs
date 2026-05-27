//! Integration tests for the `systemprompt-files` crate.
//!
//! `DATABASE_URL` must point at a Postgres instance with the full
//! systemprompt-core schema applied (run `systemprompt-test-migrate` first).

#[cfg(test)]
mod bootstrap;
#[cfg(test)]
mod config_paths;
#[cfg(test)]
mod repository;
#[cfg(test)]
mod services;
#[cfg(test)]
mod file_ingestion;
#[cfg(test)]
mod upload_service;
