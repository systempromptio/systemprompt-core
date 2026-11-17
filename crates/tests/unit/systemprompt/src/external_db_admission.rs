//! A cloud profile that reaches its database directly (`external_db_access`)
//! is admitted against that profile on the direct-database path: a disabled
//! member's command, `Direct` routing included, is refused with
//! `CommandDisabled`, and a kernel command (`infra db migrate`) admits a
//! composition without the disabled member, so its schema is not installed.
//! Only a run with no profile at all admits every linked member (K-SEC4-3).

use systemprompt::reference;
use systemprompt_cli::CliRunError;
use systemprompt_cli::kernel::{CommandRegistry, ResolvedCommand, kernel_commands};
use systemprompt_cli::runner::admit_direct;
use systemprompt_extension::Admitted;
use systemprompt_extension::cli::CommandDecl;
use systemprompt_manifest::ExtensionsConfig;
use systemprompt_traits::BoxedSource;

use crate::cli_probe::{DirectProbeGroup, ProbeGroup};
use crate::probe::{Probe, profile};

fn declared_probe() -> systemprompt::Declared {
    reference()
        .with(Probe::new("probe-cli", |c| {
            c.command(CommandDecl::group::<ProbeGroup>());
            c.command(CommandDecl::group::<DirectProbeGroup>());
        }))
        .declare()
        .expect("the probe group is declared")
}

fn resolve_direct() -> ResolvedCommand {
    let declared = declared_probe();
    let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the probe group");
    let matches = systemprompt_cli::args::command(&registry)
        .expect("the probe group attaches")
        .try_get_matches_from(["systemprompt", "probe-direct", "status"])
        .expect("the probe command parses");
    registry
        .resolve(&matches)
        .expect("the probe command resolves")
        .expect("the probe command is a contributed command")
}

fn admit_profiled(extensions: Option<&ExtensionsConfig>) -> Result<Admitted, BoxedSource> {
    let extensions = extensions.expect("a loaded profile is passed to admission");
    declared_probe()
        .admit(extensions)
        .map_err(|error| Box::new(error) as BoxedSource)
}

fn holds_probe(admitted: &Admitted) -> bool {
    admitted
        .built()
        .members()
        .any(|member| member.owner.as_str() == "probe-cli")
}

#[test]
fn a_disabled_members_direct_command_is_refused_on_a_loaded_profile() {
    let resolved = resolve_direct();
    let error = admit_direct(
        Some(&resolved),
        Some(&profile(&["probe-cli"])),
        Box::new(|_| panic!("a disabled member's command is refused before admission")),
    )
    .expect_err("the profile disables the command's owner");
    assert!(matches!(
        error,
        CliRunError::CommandDisabled { owner, .. } if owner.as_str() == "probe-cli"
    ));
}

#[test]
fn an_enabled_members_direct_command_runs_on_a_loaded_profile() {
    let resolved = resolve_direct();
    let admitted = admit_direct(
        Some(&resolved),
        Some(&profile(&[])),
        Box::new(admit_profiled),
    )
    .expect("negative control: an enabled member's command runs");
    assert!(holds_probe(&admitted));
}

#[test]
fn a_kernel_command_on_a_loaded_profile_drops_the_disabled_member() {
    let admitted = admit_direct(
        None,
        Some(&profile(&["probe-cli"])),
        Box::new(admit_profiled),
    )
    .expect("a kernel command admits against the profile");
    assert!(
        !holds_probe(&admitted),
        "the disabled member is not admitted, so migrate installs none of its schema"
    );
    let enabled = admit_direct(None, Some(&profile(&[])), Box::new(admit_profiled))
        .expect("negative control: the enabled profile admits the member");
    assert!(holds_probe(&enabled));
}

#[test]
fn with_no_profile_a_direct_command_admits_every_linked_member() {
    let resolved = resolve_direct();
    let admitted = admit_direct(
        Some(&resolved),
        None,
        Box::new(|profile| {
            assert!(profile.is_none());
            declared_probe()
                .admit_unprofiled()
                .map_err(|error| Box::new(error) as BoxedSource)
        }),
    )
    .expect("a bare --database-url run admits the full composition");
    assert!(holds_probe(&admitted));
}
