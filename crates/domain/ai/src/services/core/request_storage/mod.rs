//! Persistence pipeline for AI request audit rows.
//!
//! Builds the record from request + response, then invokes
//! [`crate::repository::AiRequestRepository`] asynchronously so the hot path
//! never blocks on the DB.

mod async_operations;
mod record_builder;
mod storage;

pub use storage::{RequestStorage, StoreParams};
