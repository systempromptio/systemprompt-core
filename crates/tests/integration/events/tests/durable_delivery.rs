//! Separate process because EventRouter installs its relay pool once per
//! process.

pub use systemprompt_events_integration_tests::{
    fixture_database_url, setup_test_pool, unique_user_id,
};

#[path = "../src/durable.rs"]
mod durable;
