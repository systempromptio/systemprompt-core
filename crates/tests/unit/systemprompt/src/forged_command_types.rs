use systemprompt_cli::kernel::{CommandRegistry, RegistryError, kernel_commands};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::admitted::{AdmittedCommand, DeclaredCommandParts};
use systemprompt_extension::cli::{CommandDecl, KernelGrant};
use systemprompt_extension::reserved::CoreId;
use systemprompt_identifiers::ExtensionId;

use crate::cli_probe::CloudGroup;

fn forged(core: Option<CoreId>) -> DeclaredCommands {
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: vec![AdmittedCommand {
            owner: ExtensionId::new(CoreId::Cloud.id()),
            core,
            decl: CommandDecl::group::<CloudGroup>().requesting(&[KernelGrant::BootProfile]),
        }],
    })
}

fn refused_as_cloud(error: &RegistryError) -> bool {
    matches!(
        error,
        RegistryError::ReservedPath {
            owner: CoreId::Cloud,
            ..
        }
    )
}

#[test]
fn a_core_id_without_core_standing_is_refused_its_reserved_path() {
    let error = CommandRegistry::build(&forged(None), &kernel_commands())
        .expect_err("the cloud id without the cloud standing cannot hold the cloud path");
    assert!(refused_as_cloud(&error), "{error:?}");
}

#[test]
fn a_core_id_stamped_with_another_core_is_refused_its_reserved_path() {
    let error = CommandRegistry::build(&forged(Some(CoreId::AnalyticsHttp)), &kernel_commands())
        .expect_err("the analytics standing cannot hold the cloud path");
    assert!(refused_as_cloud(&error), "{error:?}");
}

#[test]
fn a_core_id_stamped_with_its_own_core_holds_its_reserved_path() {
    CommandRegistry::build(&forged(Some(CoreId::Cloud)), &kernel_commands())
        .expect("the cloud standing holds the cloud path and its grants");
}
