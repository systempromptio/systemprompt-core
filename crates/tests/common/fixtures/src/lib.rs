//! Shared test fixtures for systemprompt-core test crates.

pub mod app_context;
pub mod bootstrap;
pub mod credential;
pub mod db;
pub mod jwt;
pub mod oauth;
pub mod secrets;
pub mod user;

pub use app_context::{fixture_app_context, fixture_config};
pub use bootstrap::{TestBootstrap, ensure_test_bootstrap};
pub use credential::{
    AuthedFixture, seed_admin_credential, seed_bridge_credential, seed_user_row, seed_user_session,
};
pub use db::{fixture_database_url, fixture_db_pool};
pub use jwt::{install_test_signing_key, mint_admin_jwt, mint_bridge_jwt};
pub use oauth::{OAuthClientFixture, PkcePair, pkce_pair, seed_oauth_client};
pub use secrets::ensure_test_secrets_bootstrap;
pub use user::{fixture_actor, fixture_system_admin, fixture_user_id, unique_user_id};
