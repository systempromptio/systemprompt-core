//! Integration tests for the `systemprompt-files` crate.
//!
//! `DATABASE_URL` must point at a Postgres instance with the full
//! systemprompt-core schema applied (run `systemprompt-test-migrate` first).

#[cfg(test)]
mod repository;

#[cfg(test)]
mod services;
