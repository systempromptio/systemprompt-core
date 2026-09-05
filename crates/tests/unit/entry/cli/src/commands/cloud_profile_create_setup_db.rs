//! The branch decision inside `handle_local_tenant_setup`.
//!
//! The sibling suite covers the decline and the unreachable-database paths.
//! What is left is the branch decision itself: reaching the migrations question
//! at all proves the connection probe reported the database as verified.
//!
//! Accepting migrations is deliberately not driven — the runner re-executes
//! `current_exe`, which under a test harness is the test binary, and it then
//! runs the whole suite again.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::cloud::profile::handle_local_tenant_setup;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url};

#[tokio::test]
async fn a_reachable_database_reaches_the_migrations_question() {
    let boot = ensure_test_bootstrap();
    let url = fixture_database_url().expect("a test database url");

    // Nothing is scripted, so the prompt itself is the observable: the error
    // names it only if the connection probe reported the database as verified.
    let err = handle_local_tenant_setup(
        &ScriptedPrompter::new(Vec::<String>::new()),
        &url,
        "tenant-with-a-live-database",
        &boot.profile_path,
    )
    .await
    .expect_err("with no answer scripted the migrations prompt must surface");

    assert!(
        format!("{err:#}").contains("Run database migrations?"),
        "a verified connection must ask about migrations, got: {err:#}"
    );
}
