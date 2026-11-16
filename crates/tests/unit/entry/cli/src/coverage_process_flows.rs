use clap::Parser;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::CliConfig;

use crate::process_sandbox::Sandbox;

#[derive(Parser)]
struct Build {
    #[command(subcommand)]
    command: systemprompt_cli::build::BuildCommands,
}
fn build(args: &[&str]) -> anyhow::Result<()> {
    let command = Build::try_parse_from(std::iter::once("build").chain(args.iter().copied()))
        .unwrap()
        .command;
    systemprompt_cli::build::execute(
        command,
        &CommandContext::new(
            CliConfig::new().with_interactive(false),
            EnvOverrides::default(),
        ),
    )
    .map(drop)
}
#[test]
fn coverage_core_build_forwards_release_and_offline_to_the_project_workspace() {
    let sandbox = Sandbox::new();
    sandbox.executable(
        "cargo",
        "printf '%s\\n' \"$PWD\" \"$*\" \"$SQLX_OFFLINE\" > calls",
    );
    build(&["core", "--release", "--offline"]).unwrap();
    let calls = sandbox.calls();
    assert!(calls.contains(sandbox.root.path().to_str().unwrap()));
    assert!(
        calls.contains("build --workspace --release\ntrue"),
        "{calls}"
    );
}
#[test]
fn coverage_core_debug_build_does_not_enable_release() {
    let sandbox = Sandbox::new();
    sandbox.executable("cargo", "printf '%s\\n' \"$*\" > calls");
    build(&["core"]).unwrap();
    assert_eq!(sandbox.calls().trim(), "build --workspace");
}
#[test]
fn coverage_core_build_reports_failed_compilation() {
    let sandbox = Sandbox::new();
    sandbox.executable("cargo", "exit 17");
    assert!(format!("{:#}", build(&["core"]).unwrap_err()).contains("Cargo build failed"));
}
#[test]
fn coverage_core_build_reports_missing_cargo() {
    let _sandbox = Sandbox::new();
    assert!(
        format!("{:#}", build(&["core"]).unwrap_err()).contains("Failed to execute cargo build")
    );
}
