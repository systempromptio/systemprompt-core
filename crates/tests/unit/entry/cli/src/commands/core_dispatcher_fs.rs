//! Tests that drive the profile-backed hooks arm of the `core` command group
//! through `core::execute` over the bootstrap fixture's services tree.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::{self, CoreCommands};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{CliConfig, OutputFormat};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: CoreCommands,
}

fn parse(args: &[&str]) -> CoreCommands {
    Harness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn ctx() -> CommandContext {
    crate::boot_ctx::new_with_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    core::execute(parse(args), &ctx()).await.map(drop)
}

#[tokio::test]
async fn hooks_arms_list_and_validate_without_hook_definitions() {
    run(&["hooks", "list"]).await.unwrap();
    run(&["hooks", "validate"]).await.unwrap();
}
