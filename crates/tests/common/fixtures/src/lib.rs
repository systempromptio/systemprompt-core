//! Shared test fixtures for systemprompt-core test crates.

pub mod app_context;
pub mod bootstrap;
pub mod credential;
pub mod db;
pub mod disposable_db;
pub mod jwt;
pub mod keys;
pub mod messaging;
pub mod net;
pub mod oauth;
pub mod paths;
pub mod secrets;
pub mod service_row;
pub mod skip;
pub mod subprocess;
pub mod usage;
pub mod user;
pub mod web_config;

pub use app_context::{
    fixture_app_context, fixture_app_context_with, fixture_app_context_with_config,
    fixture_app_context_with_hook, fixture_config,
};
pub use bootstrap::{
    ensure_messaging_bootstrap, ensure_test_bootstrap, init_isolated_bootstrap,
    init_services_bootstrap, init_unloadable_services_bootstrap, refresh_services_config,
    test_messaging_agent, TestBootstrap, TEST_SLACK_BOT_TOKEN, TEST_SLACK_SIGNING_SECRET,
    TEST_SLACK_WORKSPACE_ID, TEST_TEAMS_APP_ID, TEST_TEAMS_APP_PASSWORD, TEST_TEAMS_TENANT_ID,
};
pub use credential::{
    seed_admin_credential, seed_bridge_credential, seed_user_row, seed_user_row_with_roles,
    seed_user_session, AuthedFixture,
};
pub use db::{closed_db_pool, fixture_database_url, fixture_database_url_opt, fixture_db_pool};
pub use disposable_db::DisposableDb;
pub use jwt::{install_test_signing_key, mint_admin_jwt, mint_bridge_jwt};
pub use keys::{next_test_key, test_key, AUTHORITY_KEY_INDEX, ROTATING_KEY_COUNT};
pub use messaging::{agent_error_response_json, agent_reply_response_json, seed_agent_backend};
pub use net::{bind_in_range, free_port_in_range, port_is_unheld};
pub use oauth::{
    pkce_pair, seed_oauth_client, OAuthClientFixture, PkcePair, TEST_CLIENT_SECRET,
    TEST_CLIENT_SECRET_HASH, TEST_REDIRECT_URI,
};
pub use paths::{repo_path, repo_root};
pub use secrets::ensure_test_secrets_bootstrap;
pub use service_row::seed_running_service;
pub use skip::{ci, skip_or_panic};
pub use subprocess::{
    announce_helper_ready, helper, spawn_marked_child, Helper, MarkedChild, HELPER_READY_ENV,
};
pub use usage::{usage, usage_update, UsageBuilder};
pub use user::{fixture_actor, fixture_system_admin, fixture_user_id, unique_user_id};
pub use web_config::{web_config, WEB_CONFIG_YAML};
