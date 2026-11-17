//! A contributed top-level group that declares no `--help` slot is listed
//! after every kernel group and before clap's `help`, in composition order.

use systemprompt::reference;
use systemprompt_cli::kernel::{CommandRegistry, kernel_commands};
use systemprompt_extension::cli::CommandDecl;

use crate::cli_probe::{DirectProbeGroup, ProbeGroup};
use crate::probe::Probe;

fn listed_groups() -> Vec<String> {
    let declared = reference()
        .with(Probe::new("probe-cli", |c| {
            c.command(CommandDecl::group::<ProbeGroup>());
            c.command(CommandDecl::group::<DirectProbeGroup>());
        }))
        .declare()
        .expect("the probe groups are declared");
    let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the probe groups");
    let help = systemprompt_cli::args::command(&registry)
        .expect("the probe groups attach")
        .render_help()
        .to_string();
    help.lines()
        .skip_while(|line| !line.starts_with("Commands:"))
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn unslotted_groups_list_before_help_in_composition_order() {
    assert_eq!(
        listed_groups(),
        [
            "core",
            "infra",
            "admin",
            "cloud",
            "analytics",
            "web",
            "plugins",
            "build",
            "probe-tools",
            "probe-direct",
            "help",
        ]
    );
}
