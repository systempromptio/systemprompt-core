#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::config::paths::{self, PathsCommands};
use systemprompt_cli::admin::config::rate_limits::{self, RateLimitsCommands};
use systemprompt_cli::core::files::{self, FilesCommands};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_config::LoadedProfile;
use systemprompt_extension::cli::{CommandError, CommandOutput};

use crate::services_profile_fixture as fx;

fn context(tree: &fx::ProfileTree) -> CommandContext {
    let loaded = LoadedProfile::from_path(&tree.profile_path).expect("profile loads");
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
    .with_profile(Some(loaded))
}

#[test]
fn paths_validate_fails_when_a_required_path_is_missing() {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    paths::execute(PathsCommands::Validate, &context(&tree)).expect("every path exists");

    std::fs::remove_dir_all(tree.root.join("bin")).expect("drop bin");
    let err = paths::execute(PathsCommands::Validate, &context(&tree))
        .expect_err("a missing required path must fail the command");
    assert!(format!("{err:#}").contains("missing"), "{err:#}");
}

#[test]
fn rate_limits_validate_fails_on_a_zero_rate_the_loader_lets_through_while_disabled() {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    let prompter = ScriptedPrompter::default();
    rate_limits::execute(RateLimitsCommands::Validate, &prompter, &context(&tree))
        .expect("the fixture limits are valid");

    let yaml = std::fs::read_to_string(&tree.profile_path).expect("profile");
    std::fs::write(
        &tree.profile_path,
        yaml.replace("  disabled: false\n", "  disabled: true\n")
            .replace("  tasks_per_second: 50\n", "  tasks_per_second: 0\n"),
    )
    .expect("zero a rate");
    let err = rate_limits::execute(RateLimitsCommands::Validate, &prompter, &context(&tree))
        .expect_err("a zero rate must fail the command");
    assert!(format!("{err:#}").contains("errors"), "{err:#}");
}

#[derive(Debug, Parser)]
struct FilesHarness {
    #[command(subcommand)]
    command: FilesCommands,
}

#[tokio::test]
async fn files_validate_fails_for_a_file_the_upload_policy_rejects() {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let ctx = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
    .with_boot(Some(boot.boot.clone()));
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("payload.exe");
    std::fs::write(&file, b"MZ").expect("write");

    let cmd = FilesHarness::try_parse_from(["files", "validate", file.to_str().expect("utf-8")])
        .expect("parse")
        .command;
    let err = files::execute(cmd, &ctx)
        .await
        .expect_err("a rejected file must fail the command");
    assert!(format!("{err:#}").contains("validation"), "{err:#}");
}

fn carried_output(err: &anyhow::Error) -> Option<&CommandOutput> {
    err.downcast_ref::<CommandError>()
        .and_then(CommandError::output)
}

#[test]
fn a_failing_paths_validate_carries_its_report_once_and_a_passing_one_returns_it() {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    let passing =
        paths::execute(PathsCommands::Validate, &context(&tree)).expect("every path exists");
    assert_eq!(passing.title(), Some("Paths Validation"));
    assert!(!passing.should_skip_render());

    std::fs::remove_dir_all(tree.root.join("bin")).expect("drop bin");
    let err = paths::execute(PathsCommands::Validate, &context(&tree))
        .expect_err("a missing required path must fail the command");
    let report = carried_output(&err).expect("the failure carries the validation report");
    assert_eq!(report.title(), Some("Paths Validation"));
    assert_eq!(report.notes().len(), 1);
    assert_eq!(report.notes()[0].level, "error");
}

#[test]
fn a_failing_rate_limits_validate_carries_its_report() {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    let prompter = ScriptedPrompter::default();
    let passing = rate_limits::execute(RateLimitsCommands::Validate, &prompter, &context(&tree))
        .expect("the fixture limits are valid");
    assert_eq!(passing.notes()[0].level, "success");

    let yaml = std::fs::read_to_string(&tree.profile_path).expect("profile");
    std::fs::write(
        &tree.profile_path,
        yaml.replace("  disabled: false\n", "  disabled: true\n")
            .replace("  tasks_per_second: 50\n", "  tasks_per_second: 0\n"),
    )
    .expect("zero a rate");
    let err = rate_limits::execute(RateLimitsCommands::Validate, &prompter, &context(&tree))
        .expect_err("a zero rate must fail the command");
    let report = carried_output(&err).expect("the failure carries the validation report");
    assert_eq!(report.notes()[0].level, "error");
}
