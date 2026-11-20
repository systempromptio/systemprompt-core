use systemprompt_cli::kernel::{CommandRegistry, RegistryError, kernel_commands};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::admitted::{AdmittedCommand, DeclaredCommandParts};
use systemprompt_extension::cli::{CommandDecl, KernelGrant};
use systemprompt_extension::reserved::{CoreId, RESERVED_CLI_PATHS};
use systemprompt_identifiers::ExtensionId;

use crate::cli_probe::CloudGroup;

fn rows_listing(grant: KernelGrant) -> Vec<(CoreId, Vec<&'static str>)> {
    RESERVED_CLI_PATHS
        .iter()
        .filter(|row| row.grants.contains(&grant))
        .map(|row| (row.owner, row.path.to_vec()))
        .collect()
}

#[test]
fn the_services_tree_grant_is_listed_only_on_the_marketplace_paths() {
    assert_eq!(
        rows_listing(KernelGrant::ServicesTree),
        vec![
            (CoreId::MarketplaceHttp, vec!["core", "marketplace"]),
            (CoreId::MarketplaceHttp, vec!["core", "plugins"]),
            (CoreId::MarketplaceHttp, vec!["core", "skills"]),
        ]
    );
}

#[test]
fn the_caller_session_grant_is_listed_only_on_plugins_mcp() {
    assert_eq!(
        rows_listing(KernelGrant::CallerSession),
        vec![(CoreId::McpHttp, vec!["plugins", "mcp"])]
    );
}

#[test]
fn the_boot_profile_grant_is_listed_only_on_cloud() {
    assert_eq!(
        rows_listing(KernelGrant::BootProfile),
        vec![(CoreId::Cloud, vec!["cloud"])]
    );
}

fn forged_cloud(grant: KernelGrant) -> DeclaredCommands {
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: vec![AdmittedCommand {
            owner: ExtensionId::new(CoreId::Cloud.id()),
            core: Some(CoreId::Cloud),
            decl: CommandDecl::group::<CloudGroup>().requesting(&[grant]),
        }],
    })
}

#[test]
fn a_core_path_requesting_a_grant_its_row_does_not_list_is_refused() {
    for grant in [KernelGrant::ServicesTree, KernelGrant::CallerSession] {
        let error = CommandRegistry::build(&forged_cloud(grant), &kernel_commands())
            .expect_err("the cloud row lists neither grant");
        assert!(
            matches!(error, RegistryError::GrantNotListed { grant: refused, .. } if refused == grant),
            "{error:?}"
        );
    }
}

#[test]
fn a_core_path_requesting_a_grant_its_row_lists_is_admitted() {
    CommandRegistry::build(&forged_cloud(KernelGrant::BootProfile), &kernel_commands())
        .expect("negative control: the cloud row lists boot-profile");
}
