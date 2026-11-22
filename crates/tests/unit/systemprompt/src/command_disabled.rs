//! A command whose owner the profile disables is refused at dispatch with
//! `CommandDisabled`, decided from the profile and the command's stamped
//! owner, while the same command under a profile that enables it passes (T23).
//! With no profile (a bare `--database-url` run) a member's command whose
//! declaration holds no `CommandClaim::DirectDatabase` is refused with
//! `ProfileRequired`, while one whose row confers it (`analytics`), and a
//! kernel command, admit every linked member.

use systemprompt::reference;
use systemprompt_cli::CliRunError;
use systemprompt_cli::kernel::{CommandRegistry, ResolvedCommand, kernel_commands};
use systemprompt_cli::runner::{admit_unprofiled, refuse_disabled};
use systemprompt_extension::cli::CommandDecl;

use crate::cli_probe::ProbeGroup;
use crate::probe::{Probe, profile};

fn declared_probe() -> systemprompt::Declared {
    reference()
        .with(Probe::new("probe-cli", |c| {
            c.command(CommandDecl::group::<ProbeGroup>());
        }))
        .declare()
        .expect("the probe group is declared")
}

fn resolved_probe_command() -> ResolvedCommand {
    resolve_probe("probe-tools")
}

fn resolve_probe(group: &str) -> ResolvedCommand {
    resolve_line(&[group, "status"])
}

fn resolve_line(line: &[&str]) -> ResolvedCommand {
    let declared = declared_probe();
    let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the probe group");
    let matches = systemprompt_cli::args::command(&registry)
        .expect("the probe group attaches")
        .try_get_matches_from(std::iter::once("systemprompt").chain(line.iter().copied()))
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

#[test]
fn a_member_command_with_no_profile_requires_one() {
    let resolved = resolved_probe_command();
    let error = admit_unprofiled(
        Some(&resolved),
        Box::new(|_| panic!("a member command never admits without a profile")),
    )
    .expect_err("a member command needs a profile");
    assert!(matches!(
        error,
        CliRunError::ProfileRequired { path } if path == ["probe-tools"]
    ));
}

fn admit_full(
    profile: Option<&systemprompt_manifest::ExtensionsConfig>,
) -> Result<systemprompt_extension::Admitted, systemprompt_traits::BoxedSource> {
    assert!(
        profile.is_none(),
        "a bare --database-url run has no profile"
    );
    declared_probe()
        .admit_unprofiled()
        .map_err(|error| Box::new(error) as systemprompt_traits::BoxedSource)
}

#[test]
fn a_direct_member_command_with_no_profile_admits_the_full_composition() {
    let resolved = resolve_line(&["analytics", "overview"]);
    let admitted = admit_unprofiled(Some(&resolved), Box::new(admit_full))
        .expect("a member command its row confers direct-database runs without a profile");
    assert!(
        admitted
            .built()
            .members()
            .any(|member| member.owner.as_str() == "probe-cli"),
        "no member is disabled without a profile"
    );
}

#[test]
fn a_kernel_command_with_no_profile_admits_the_full_composition() {
    let admitted = admit_unprofiled(
        None,
        Box::new(|profile| {
            assert!(
                profile.is_none(),
                "a bare --database-url run has no profile"
            );
            declared_probe()
                .admit_unprofiled()
                .map_err(|error| Box::new(error) as systemprompt_traits::BoxedSource)
        }),
    )
    .expect("a kernel command admits without a profile");
    assert!(
        admitted
            .built()
            .members()
            .any(|member| member.owner.as_str() == "probe-cli"),
        "no member is disabled without a profile"
    );
}

#[test]
fn a_profile_still_refuses_a_disabled_members_command() {
    let resolved = resolved_probe_command();
    assert!(matches!(
        refuse_disabled(&resolved, &profile(&["probe-cli"])),
        Err(CliRunError::CommandDisabled { .. })
    ));
}
