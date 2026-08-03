//! Tests that drive `cloud profile show` and `cloud profile list` through the
//! profile command dispatcher.
//!
//! The projection each `--filter` builds is otherwise never invoked; the
//! bootstrap fixture supplies the runtime config these projections read.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::profile::{self, ProfileCommands, ShowFilter};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

fn ctx() -> CommandContext {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    // `show` resolves its profile from the CLI flag, then the env override,
    // then discovery; the fixture profile lives in a tempdir that discovery
    // cannot see, so point the env override at it.
    let env = EnvOverrides {
        profile: Some(boot.profile_path.to_string_lossy().to_string()),
        ..EnvOverrides::default()
    };
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        env,
    )
}

fn show(filter: ShowFilter, json: bool, yaml: bool) -> ProfileCommands {
    ProfileCommands::Show {
        name: None,
        filter,
        json,
        yaml,
    }
}

#[tokio::test]
async fn show_renders_every_filter_projection() {
    let ctx = ctx();

    for filter in [
        ShowFilter::All,
        ShowFilter::Agents,
        ShowFilter::Mcp,
        ShowFilter::Skills,
        ShowFilter::Ai,
        ShowFilter::Web,
        ShowFilter::Content,
        ShowFilter::Env,
        ShowFilter::Settings,
    ] {
        profile::execute(Some(show(filter, false, false)), &ctx)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn show_honours_the_json_and_yaml_output_flags() {
    let ctx = ctx();

    profile::execute(Some(show(ShowFilter::All, true, false)), &ctx)
        .await
        .unwrap();
    profile::execute(Some(show(ShowFilter::All, false, true)), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn show_rejects_a_profile_name_that_does_not_resolve() {
    let ctx = ctx();

    let err = profile::execute(
        Some(ProfileCommands::Show {
            name: Some("cov_no_such_profile".to_owned()),
            filter: ShowFilter::All,
            json: false,
            yaml: false,
        }),
        &ctx,
    )
    .await
    .unwrap_err();

    assert!(format!("{err:#}").contains("cov_no_such_profile"));
}

#[tokio::test]
async fn list_enumerates_the_discoverable_profiles() {
    let ctx = ctx();

    profile::execute(Some(ProfileCommands::List), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_missing_subcommand_is_refused_without_a_terminal() {
    let ctx = ctx();

    let err = profile::execute(None, &ctx).await.unwrap_err();
    assert!(format!("{err:#}").contains("Profile subcommand required"));
}
