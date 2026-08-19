//! Tests for the post-create local tenant setup step.
//!
//! `handle_local_tenant_setup` probes the tenant database and then offers
//! migrations; these drive the reachable-database and unreachable-database
//! branches. The migration branch is deliberately declined —
//! `run_migrations_cmd` re-executes `current_exe`, which under a test harness
//! is the test binary itself.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::cloud::profile::handle_local_tenant_setup;
use systemprompt_test_fixtures::fixture_database_url;

#[tokio::test]
async fn a_reachable_database_offers_migrations_and_accepts_a_decline() {
    let url = fixture_database_url().unwrap();
    let prompter = ScriptedPrompter::new(["no"]);
    let profile_path = std::path::Path::new("/nonexistent/profile.yaml");

    handle_local_tenant_setup(&prompter, &url, "local", profile_path)
        .await
        .unwrap();
}

#[tokio::test]
async fn an_unreachable_database_without_a_compose_file_only_warns() {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let profile_path = std::path::Path::new("/nonexistent/profile.yaml");

    handle_local_tenant_setup(
        &prompter,
        "postgres://nobody:nothing@127.0.0.1:1/absent",
        "tenant_without_compose_fixture",
        profile_path,
    )
    .await
    .unwrap();
}
