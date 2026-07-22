//! Shared test fixtures for systemprompt-core test crates.

pub mod app_context;
pub mod bootstrap;
pub mod credential;
pub mod db;
pub mod jwt;
pub mod messaging;
pub mod oauth;
pub mod secrets;
pub mod user;
pub mod web_config;

pub use app_context::{
    fixture_app_context, fixture_app_context_with, fixture_app_context_with_config,
    fixture_app_context_with_hook, fixture_config,
};
pub use bootstrap::{
    ensure_messaging_bootstrap, ensure_test_bootstrap, test_messaging_agent, TestBootstrap,
    TEST_SLACK_BOT_TOKEN, TEST_SLACK_SIGNING_SECRET, TEST_SLACK_WORKSPACE_ID, TEST_TEAMS_APP_ID,
    TEST_TEAMS_APP_PASSWORD, TEST_TEAMS_TENANT_ID,
};
pub use credential::{
    seed_admin_credential, seed_bridge_credential, seed_user_row, seed_user_row_with_roles,
    seed_user_session, AuthedFixture,
};
pub use db::{closed_db_pool, fixture_database_url, fixture_db_pool};
pub use jwt::{install_test_signing_key, mint_admin_jwt, mint_bridge_jwt};
pub use messaging::{agent_error_response_json, agent_reply_response_json, seed_agent_backend};
pub use oauth::{
    pkce_pair, seed_oauth_client, OAuthClientFixture, PkcePair, TEST_CLIENT_SECRET,
    TEST_REDIRECT_URI,
};
pub use secrets::ensure_test_secrets_bootstrap;
pub use user::{fixture_actor, fixture_system_admin, fixture_user_id, unique_user_id};
pub use web_config::{web_config, WEB_CONFIG_YAML};
