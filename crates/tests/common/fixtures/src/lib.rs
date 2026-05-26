//! Shared test fixtures for systemprompt-core test crates.

pub mod app_context;
pub mod db;
pub mod secrets;
pub mod user;

pub use app_context::{fixture_app_context, fixture_config};
pub use db::{fixture_database_url, fixture_db_pool};
pub use secrets::ensure_test_secrets_bootstrap;
pub use user::{fixture_actor, fixture_system_admin, fixture_user_id, unique_user_id};
