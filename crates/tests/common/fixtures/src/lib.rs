//! Shared test fixtures for systemprompt-core test crates.
//!
//! A test that needs a database or a DB-backed fixture takes it from the
//! panicking helpers — [`test_database_url`], [`test_db_pool`],
//! [`test_pg_pool`], [`DisposableDb::empty`], [`DisposableDb::with_schema`],
//! [`test_app_context`]. A missing prerequisite fails the test instead of
//! skipping it, because a skipped test reports the same green as one that ran
//! and CI and `just test-shard` always provide `DATABASE_URL`. There is no
//! skip-on-missing form.

pub mod agent;
pub mod app_context;
pub mod bootstrap;
pub mod catalog;
pub mod credential;
pub mod db;
pub mod disposable_db;
pub mod instance;
pub mod jwt;
pub mod keys;
pub mod liveness;
pub mod messaging;
pub mod net;
pub mod oauth;
pub mod orphans;
pub mod paths;
pub mod schema;
pub mod secrets;
pub mod service_row;
pub mod subprocess;
pub mod usage;
pub mod user;
pub mod web_config;

pub use agent::{
    NotManagedSkills, ScriptedSkills, ToolExecutionLedger, a2a_dependencies, a2a_repositories,
    not_managed_skills, scripted_skills, tool_execution_ledger,
};
pub use app_context::{
    FIXTURE_JWT_ISSUER, default_governance_engine, default_governance_watch,
    fixture_analytics_repositories, fixture_app_context_with, fixture_app_context_with_config,
    fixture_app_context_with_extensions, fixture_app_context_with_hook,
    fixture_app_context_with_user_repository, fixture_artifact_ingest, fixture_config,
    fixture_fingerprint_repository, offline_rate_windows, test_app_context,
};
pub use bootstrap::{
    TEST_SLACK_BOT_TOKEN, TEST_SLACK_SIGNING_SECRET, TEST_SLACK_WORKSPACE_ID, TEST_TEAMS_APP_ID,
    TEST_TEAMS_APP_PASSWORD, TEST_TEAMS_TENANT_ID, TEST_TEAMS_UNRESOLVABLE_APP_ID,
    TEST_TEAMS_UNRESOLVABLE_TENANT_ID, TestBootstrap, ensure_messaging_bootstrap,
    ensure_test_bootstrap, init_isolated_bootstrap, init_services_bootstrap,
    init_unloadable_services_bootstrap, messaging_config_yaml_with_teams_endpoints,
    refresh_services_config, test_messaging_agent,
};
pub use credential::{
    AuthedFixture, seed_admin_credential, seed_bridge_credential, seed_fixture_system_admin,
    seed_user_row, seed_user_row_with_roles, seed_user_session,
};
pub use db::{closed_db_pool, lazy_pg_pool, test_database_url, test_db_pool, test_pg_pool};
pub use disposable_db::DisposableDb;
pub use instance::unique_instance;
pub use jwt::{install_test_signing_key, mint_admin_jwt, mint_bridge_jwt};
pub use keys::{AUTHORITY_KEY_INDEX, ROTATING_KEY_COUNT, next_test_key, test_key};
pub use liveness::LivePids;
pub use messaging::{agent_error_response_json, agent_reply_response_json, seed_agent_backend};
pub use net::{bind_in_range, free_port_in_range, port_is_unheld};
pub use oauth::{
    OAuthClientFixture, PkcePair, TEST_CLIENT_SECRET, TEST_CLIENT_SECRET_HASH, TEST_REDIRECT_URI,
    pkce_pair, seed_oauth_client,
};
pub use paths::{canonical_tempdir, repo_path, repo_root};
pub use schema::{install_extension_schemas, install_extension_schemas_with_config};
pub use secrets::{ensure_test_secrets_bootstrap, install_named_secret};
pub use service_row::seed_running_service;
pub use subprocess::{
    HELPER_READY_ENV, Helper, MarkedChild, announce_helper_ready, helper, spawn_marked_child,
};
pub use usage::{UsageBuilder, usage, usage_update};
pub use user::{
    FIXTURE_USER_ID, fixture_actor, fixture_logging_runtime, fixture_system_admin, fixture_user_id,
    merging_user_service, unique_user_id,
};
pub use web_config::{WEB_CONFIG_YAML, web_config};
