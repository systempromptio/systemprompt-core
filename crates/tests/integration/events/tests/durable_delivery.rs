//! Durable outbox delivery, compiled as its own test target over the shared
//! `src/durable.rs` module.

pub use systemprompt_events_integration_tests::{setup_test_pool, unique_user_id};

#[path = "../src/durable.rs"]
mod durable;
