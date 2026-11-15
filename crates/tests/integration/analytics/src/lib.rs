//! Integration tests for the `systemprompt-analytics` crate and the route
//! handlers of `systemprompt-analytics-entry`.
//!
//! `DATABASE_URL` must point at a Postgres instance with the full
//! systemprompt-core schema applied (run `systemprompt-test-migrate` first).

#[cfg(test)]
mod costs;

#[cfg(test)]
mod repositories;

#[cfg(test)]
mod repositories_extended;

#[cfg(test)]
mod routes;

#[cfg(test)]
mod session_pipeline;
