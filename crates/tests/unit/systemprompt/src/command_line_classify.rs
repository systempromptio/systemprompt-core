//! The host's classification of a forwarded command line fails closed: a
//! line that selects no command, or a command whose owner the profile
//! disables, is an error, while a known read-only command is described with
//! its path and owner.

use systemprompt::reference;
use systemprompt_cli::kernel::{CommandRegistry, classify_command_line, kernel_commands};
use systemprompt_extension::cli::{DataImpact, RoutingClass};

use crate::probe::profile;

const OVERVIEW: [&str; 2] = ["analytics", "overview"];

fn registry() -> CommandRegistry {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands")
}

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

#[test]
fn a_known_read_only_command_is_described_with_its_path_and_owner() {
    let described = classify_command_line(&registry(), &profile(&[]), &argv(&OVERVIEW))
        .expect("analytics overview is classified");
    assert_eq!(described.path, OVERVIEW);
    assert_eq!(
        described.owner.as_ref().map(|owner| owner.as_str()),
        Some("analytics-http")
    );
    assert_eq!(described.descriptor.routing_class(), RoutingClass::ReadOnly);
    assert_eq!(described.descriptor.data_impact(), DataImpact::Preserving);
}

#[test]
fn an_unknown_command_line_is_an_error() {
    let registry = registry();
    assert!(
        classify_command_line(&registry, &profile(&[]), &argv(&["no-such-command"])).is_err(),
        "an unknown path is never described"
    );
    assert!(
        classify_command_line(&registry, &profile(&[]), &argv(&OVERVIEW)).is_ok(),
        "negative control: a known read-only path is described"
    );
}

#[test]
fn a_disabled_members_command_line_is_an_error() {
    let registry = registry();
    assert!(
        classify_command_line(&registry, &profile(&["analytics-http"]), &argv(&OVERVIEW)).is_err(),
        "the profile disables the command's owner"
    );
    assert!(
        classify_command_line(&registry, &profile(&["content-http"]), &argv(&OVERVIEW)).is_ok(),
        "negative control: disabling another member leaves the command described"
    );
}
