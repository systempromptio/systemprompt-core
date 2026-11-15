//! A command whose owner the profile disables is refused at dispatch with
//! `CommandDisabled`, decided from the profile and the command's stamped
//! owner, while the same command under a profile that enables it passes (T23).

use clap::Command;
use systemprompt::reference;
use systemprompt_cli::CliRunError;
use systemprompt_cli::kernel::{CommandRegistry, ResolvedCommand, kernel_commands};
use systemprompt_cli::runner::refuse_disabled;
use systemprompt_extension::cli::CommandDecl;

use crate::cli_probe::ProbeGroup;
use crate::probe::{Probe, profile};

fn resolved_probe_command() -> ResolvedCommand {
    let declared = reference()
        .with(Probe::new("probe-cli", |c| {
            c.command(CommandDecl::group::<ProbeGroup>());
        }))
        .declare()
        .expect("the probe group is declared");
    let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the probe group");
    let matches = registry
        .assemble(Command::new("systemprompt"))
        .expect("the probe group attaches")
        .try_get_matches_from(["systemprompt", "probe-tools", "status"])
        .expect("the probe command parses");
    registry
        .resolve(&matches)
        .expect("the probe command resolves")
        .expect("the probe command is a contributed command")
}

#[test]
fn a_disabled_owners_command_is_refused_at_dispatch() {
    let resolved = resolved_probe_command();
    let error = refuse_disabled(&resolved, &profile(&["probe-cli"]))
        .expect_err("the profile disables the command's owner");
    assert!(matches!(
        error,
        CliRunError::CommandDisabled { path, owner }
            if path == ["probe-tools"] && owner.as_str() == "probe-cli"
    ));
}

#[test]
fn an_enabled_owners_command_passes_dispatch() {
    let resolved = resolved_probe_command();
    assert!(
        refuse_disabled(&resolved, &profile(&[])).is_ok(),
        "negative control"
    );
}

#[test]
fn disabling_another_member_leaves_the_command_alone() {
    let resolved = resolved_probe_command();
    assert!(refuse_disabled(&resolved, &profile(&["content-http"])).is_ok());
}
