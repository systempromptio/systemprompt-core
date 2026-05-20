//! Integration tests for the systemprompt scheduler.
//!
//! Per the project's testing policy, tests live in a dedicated crate. This
//! crate covers the scheduler's externally observable guarantees — most
//! importantly the cross-replica single-execution guarantee enforced by the
//! Postgres advisory lock (`distributed_lock`).

#[cfg(test)]
mod distributed_lock;

#[cfg(test)]
mod static_content_tests;
