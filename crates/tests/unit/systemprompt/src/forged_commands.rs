//! The CLI registry re-checks a forged command handoff against the reserved
//! CLI paths: a reserved path or its grant under a non-core id is refused
//! however the handoff was built (T24, B1).

use systemprompt_cli::kernel::{CommandRegistry, RegistryError, core_owner_type, kernel_commands};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::admitted::{AdmittedCommand, DeclaredCommandParts};
use systemprompt_extension::cli::{CommandDecl, KernelGrant};
use systemprompt_extension::reserved::CoreId;
use systemprompt_identifiers::ExtensionId;

use crate::cli_probe::{AnalyticsGroup, CloudGroup, ProbeGroup};

fn forged(commands: Vec<(&str, CommandDecl)>) -> DeclaredCommands {
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: commands
            .into_iter()
            .map(|(owner, decl)| AdmittedCommand {
                owner: ExtensionId::new(owner),
                owner_type: CoreId::for_id(owner)
                    .and_then(core_owner_type)
                    .unwrap_or(std::any::type_name::<ProbeGroup>()),
                decl,
            })
            .collect(),
    })
}

fn cloud() -> CommandDecl {
    CommandDecl::group::<CloudGroup>().requesting(&[KernelGrant::CloudOperator])
}

#[test]
fn forged_install_at_cloud_is_refused() {
    let error = CommandRegistry::build(
        &forged(vec![("probe-install", cloud())]),
        &kernel_commands(),
    )
    .expect_err("an install id cannot hold the cloud path");
    assert!(matches!(
        error,
        RegistryError::ReservedPath {
            owner: CoreId::Cloud,
            ..
        }
    ));
}

#[test]
fn forged_core_id_at_another_core_path_is_refused() {
    let error = CommandRegistry::build(
        &forged(vec![
            ("cloud", cloud()),
            ("cloud", CommandDecl::group::<AnalyticsGroup>()),
        ]),
        &kernel_commands(),
    )
    .expect_err("one id cannot hold two core ids' paths");
    assert!(matches!(
        error,
        RegistryError::ReservedPath {
            owner: CoreId::AnalyticsHttp,
            ..
        }
    ));
}

#[test]
fn forged_grant_on_an_unreserved_path_is_refused() {
    let decl = CommandDecl::group::<ProbeGroup>().requesting(&[KernelGrant::CloudOperator]);
    let error = CommandRegistry::build(&forged(vec![("probe-install", decl)]), &kernel_commands())
        .expect_err("a grant is listed only on its reserved path");
    assert!(matches!(
        error,
        RegistryError::GrantNotListed {
            grant: KernelGrant::CloudOperator,
            ..
        }
    ));
}

#[test]
fn core_id_at_its_own_path_is_accepted() {
    let registry = CommandRegistry::build(&forged(vec![("cloud", cloud())]), &kernel_commands())
        .expect("negative control: the cloud member holds the cloud path");
    assert!(
        registry
            .entries()
            .iter()
            .any(|entry| entry.decl().path() == ["cloud"])
    );
}
