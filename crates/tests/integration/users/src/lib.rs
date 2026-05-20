//! Integration tests for the `systemprompt-users` crate.
//!
//! `DATABASE_URL` must point at a Postgres instance with the full
//! systemprompt-core schema applied (run `systemprompt-test-migrate` first).

#[cfg(test)]
mod admin_service;

#[cfg(test)]
mod banned_ip_repository;

#[cfg(test)]
mod user_repository;

#[cfg(test)]
mod user_service;
