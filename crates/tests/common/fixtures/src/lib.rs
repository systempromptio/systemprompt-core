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
pub mod governance;
pub mod instance;
pub mod jwt;
pub mod keys;
pub mod keystore;
pub mod liveness;
pub mod messaging;
pub mod net;
pub mod oauth;
pub mod orphans;
pub mod paths;
pub mod poll;
pub mod schema;
pub mod secrets;
pub mod service_row;
pub mod subprocess;
pub mod usage;
pub mod user;
pub mod web_config;

pub use agent::{
    a2a_dependencies, a2a_repositories, not_managed_skills, scripted_skills, tool_execution_ledger,
    NotManagedSkills, ScriptedSkills, ToolExecutionLedger,
};
pub use app_context::{
    default_governance_engine, default_governance_watch, fixture_analytics_repositories,
    fixture_app_context_on_bootstrap, fixture_app_context_on_bootstrap_with_paths,
    fixture_app_context_with, fixture_app_context_with_config, fixture_app_context_with_extensions,
    fixture_app_context_with_hook, fixture_app_context_with_user_repository,
    fixture_artifact_ingest, fixture_config, fixture_event_bridge, fixture_fingerprint_repository,
    fixture_services_state, fixture_services_view, fixture_subject_providers,
    offline_rate_windows, test_app_context, test_app_context_on, test_config, test_config_with,
    FIXTURE_JWT_ISSUER,
};
pub use bootstrap::{
    ensure_messaging_bootstrap, ensure_test_bootstrap, init_isolated_bootstrap,
    init_services_bootstrap, init_unloadable_services_bootstrap,
    messaging_config_yaml_with_teams_endpoints, test_messaging_agent, test_skills_root,
    TestBootstrap, TEST_SLACK_BOT_TOKEN, TEST_SLACK_SIGNING_SECRET, TEST_SLACK_WORKSPACE_ID,
    TEST_TEAMS_APP_ID, TEST_TEAMS_APP_PASSWORD, TEST_TEAMS_TENANT_ID,
    TEST_TEAMS_UNRESOLVABLE_APP_ID, TEST_TEAMS_UNRESOLVABLE_TENANT_ID,
};
pub use credential::{
    seed_admin_credential, seed_bridge_credential, seed_fixture_system_admin, seed_user_row,
    seed_user_row_with_roles, seed_user_session, AuthedFixture,
};
pub use db::{closed_db_pool, lazy_pg_pool, test_database_url, test_db_pool, test_pg_pool};
pub use disposable_db::DisposableDb;
pub use governance::{apply_governance, apply_governance_plane, record_plane_applied};
pub use instance::{unique_agent_name, unique_instance, unique_name, unique_service_name};
pub use jwt::{mint_admin_jwt, mint_bridge_jwt, test_signing_keys};
pub use keys::{next_test_key, test_key, AUTHORITY_KEY_INDEX, ROTATING_KEY_COUNT};
pub use keystore::install_headless_keystore;
pub use liveness::LivePids;
pub use messaging::{agent_error_response_json, agent_reply_response_json, seed_agent_backend};
pub use net::{bind_in_range, free_port_in_range, port_is_unheld};
pub use oauth::{
    oauth_fixture_pepper, oauth_fixture_repository, pkce_pair, seed_oauth_client,
    OAuthClientFixture, PkcePair, TEST_CLIENT_SECRET, TEST_CLIENT_SECRET_HASH, TEST_REDIRECT_URI,
};
pub use paths::{canonical_tempdir, repo_path, repo_root};
pub use poll::{block_on, eventually};
pub use schema::{install_extension_schemas, install_extension_schemas_with_config};
pub use secrets::{ensure_test_secrets_bootstrap, fixture_at_rest_cipher, install_named_secret};
pub use service_row::seed_running_service;
pub use subprocess::{
    announce_helper_ready, bounded_output, helper, spawn_marked_child, test_spawner, Helper,
    MarkedChild, HELPER_READY_ENV,
};
pub use usage::{usage, usage_update, UsageBuilder};
pub use user::{
    fixture_actor, fixture_logging_runtime, fixture_system_admin, fixture_user_id,
    merging_user_service, unique_user_id, FIXTURE_USER_ID,
};
pub use web_config::{web_config, WEB_CONFIG_YAML};
