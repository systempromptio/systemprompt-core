//! Tests that drive the `plugins` command group through its dispatcher.
//!
//! The per-subcommand bodies are covered elsewhere; the dispatcher arms
//! themselves (render + error mapping) are only reached this way.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::plugins::{self, PluginsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_extension::ExtensionRegistry;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: PluginsCommands,
}

fn parse(args: &[&str]) -> PluginsCommands {
    Harness::try_parse_from(std::iter::once("plugins").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

fn first_extension_id() -> String {
    ExtensionRegistry::discover()
        .unwrap()
        .extensions()
        .first()
        .map(|e| e.id().to_owned())
        .expect("compiled registry must not be empty")
}

#[tokio::test]
async fn list_show_and_config_arms_render_for_a_compiled_extension() {
    let ctx = ctx();
    let id = first_extension_id();

    plugins::execute(parse(&["list"]), &ctx).await.unwrap();
    plugins::execute(parse(&["list", "--type", "compiled"]), &ctx)
        .await
        .unwrap();
    plugins::execute(parse(&["show", &id]), &ctx).await.unwrap();
    plugins::execute(parse(&["config"]), &ctx).await.unwrap();
    plugins::execute(parse(&["config", &id]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn show_arm_maps_an_unknown_extension_to_an_error() {
    let ctx = ctx();

    let err = plugins::execute(parse(&["show", "cov_no_such_extension"]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Failed to show extension"));
}

#[tokio::test]
async fn capabilities_and_validate_arms_run_over_the_registry() {
    let ctx = ctx();

    plugins::execute(parse(&["capabilities"]), &ctx)
        .await
        .unwrap();
    plugins::execute(parse(&["capabilities", "jobs"]), &ctx)
        .await
        .unwrap();
    plugins::execute(parse(&["validate"]), &ctx).await.unwrap();
}
