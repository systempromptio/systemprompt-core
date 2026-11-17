//! The kernel host the cli suites run a command against, and the owed check
//! that a profile-only command needs no database while a database command
//! without one is refused.

use clap::Parser;
use systemprompt_cli::admin::config::paths::{self, PathsCommands};
use systemprompt_cli::infrastructure::jobs::{self, JobsCommands};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_config::LoadedProfile;
use systemprompt_extension::cli::{CliConfig, OutputFormat};

use crate::services_profile_fixture as fx;

pub fn kernel_host(ctx: CommandContext) -> CommandContext {
    ctx.with_detached_kernel()
}

pub fn profile_host(profile: &std::path::Path) -> CommandContext {
    let loaded = LoadedProfile::from_path(profile).expect("profile loads");
    kernel_host(
        CommandContext::new(
            CliConfig::new()
                .with_interactive(false)
                .with_output_format(OutputFormat::Json),
            EnvOverrides::default(),
        )
        .with_profile(Some(loaded)),
    )
}

#[derive(Debug, Parser)]
struct JobsHarness {
    #[command(subcommand)]
    command: JobsCommands,
}

#[test]
fn a_profile_only_command_runs_with_no_database() {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    let host = profile_host(&tree.profile_path);
    assert!(!host.is_database_scoped());

    paths::execute(PathsCommands::Show, &host).expect("paths show reads only the profile");
}

#[tokio::test]
async fn a_database_command_without_a_database_is_refused() {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    let host = profile_host(&tree.profile_path);
    let cmd = JobsHarness::try_parse_from(["jobs", "history"])
        .expect("parse")
        .command;

    let err = jobs::execute(cmd, &host)
        .await
        .expect_err("jobs history needs a database the host does not have");
    assert!(format!("{err:#}").contains("requires"), "{err:#}");
}
