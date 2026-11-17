use systemprompt_cli::kernel::{CommandRegistry, RegistryError, core_owner_type, kernel_commands};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::admitted::{AdmittedCommand, DeclaredCommandParts};
use systemprompt_extension::cli::{CommandDecl, KernelGrant};
use systemprompt_extension::reserved::CoreId;
use systemprompt_identifiers::ExtensionId;

use crate::cli_probe::CloudGroup;

fn forged(owner_type: &'static str) -> DeclaredCommands {
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: vec![AdmittedCommand {
            owner: ExtensionId::new(CoreId::Cloud.id()),
            owner_type,
            decl: CommandDecl::group::<CloudGroup>().requesting(&[KernelGrant::BootProfile]),
        }],
    })
}

#[test]
fn the_core_owner_types_name_the_owning_extensions() {
    assert_eq!(
        core_owner_type(CoreId::Cloud),
        Some(std::any::type_name::<
            systemprompt::cloud::entry::CloudCliExtension,
        >())
    );
    assert_eq!(
        core_owner_type(CoreId::AnalyticsHttp),
        Some(std::any::type_name::<
            systemprompt::analytics::entry::extension::AnalyticsHttpExtension,
        >())
    );
}

#[test]
fn a_core_id_under_another_type_is_refused_its_reserved_path() {
    let error =
        CommandRegistry::build(&forged("probe_install::ProbeExtension"), &kernel_commands())
            .expect_err("the cloud id under a probe type cannot hold the cloud path");
    assert!(matches!(
        error,
        RegistryError::ReservedPath {
            owner: CoreId::Cloud,
            ..
        }
    ));
}

#[test]
fn a_core_id_under_its_owning_type_holds_its_reserved_path() {
    let owner = core_owner_type(CoreId::Cloud).expect("cloud has an owner type");
    CommandRegistry::build(&forged(owner), &kernel_commands())
        .expect("the owning type holds the cloud path and its grants");
}
