//! DB-backed tests for `admin bootstrap`.
//!
//! The command resolves the admin username from the runtime `Config` and
//! connects with its database URL, so the bootstrap fixture (profile + config
//! wired to the test database) is all it needs.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::bootstrap::{BootstrapArgs, execute};

fn args(name: Option<&str>) -> BootstrapArgs {
    BootstrapArgs {
        name: name.map(ToOwned::to_owned),
        email: None,
        full_name: "Coverage Admin".to_owned(),
    }
}

fn card(output: &systemprompt_cli::shared::CommandOutput) -> serde_json::Value {
    serde_json::to_value(output.artifact()).unwrap()
}

#[tokio::test]
async fn bootstrap_is_idempotent_and_grants_the_admin_role() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let config = CliConfig::new().with_interactive(false);

    let first = execute(args(None), &config).await.unwrap();
    let first_json = card(&first).to_string();
    assert!(first_json.contains("admin"), "{first_json}");

    let second = execute(args(None), &config).await.unwrap();
    let second_json = card(&second).to_string();
    assert!(
        second_json.contains("already exists"),
        "second run should verify rather than create: {second_json}"
    );

    // Passing the configured name explicitly takes the match branch rather
    // than the refusal branch.
    let configured = systemprompt_models::Config::get()
        .unwrap()
        .system_admin_username
        .clone();
    let third = execute(args(Some(&configured)), &config).await.unwrap();
    assert!(card(&third).to_string().contains(&configured));
}

#[tokio::test]
async fn bootstrap_refuses_a_name_that_is_not_the_configured_admin() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let config = CliConfig::new().with_interactive(false);

    let err = execute(args(Some("someone_else")), &config)
        .await
        .unwrap_err();

    assert!(format!("{err:#}").contains("refusing to bootstrap the wrong user"));
}
