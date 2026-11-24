use systemprompt_cli::kernel::{CommandRegistry, RegistryError, kernel_commands};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::admitted::{AdmittedCommand, DeclaredCommandParts};
use systemprompt_extension::cli::{CommandDecl, GrantedPlane, KernelGrant};
use systemprompt_extension::reserved::{CoreId, RESERVED_CLI_PATHS};
use systemprompt_identifiers::ExtensionId;

use crate::cli_probe::{CloudGroup, ProbeGroup};

fn rows_listing(grant: KernelGrant) -> Vec<(CoreId, Vec<&'static str>)> {
    RESERVED_CLI_PATHS
        .iter()
        .filter(|row| row.grants.contains(&grant))
        .map(|row| (row.owner, row.path.to_vec()))
        .collect()
}

#[test]
fn the_services_tree_grant_is_listed_only_on_the_agent_marketplace_and_mcp_paths() {
    assert_eq!(
        rows_listing(KernelGrant::ServicesTree),
        vec![
            (CoreId::AgentHttp, vec!["admin", "agents"]),
            (CoreId::MarketplaceHttp, vec!["core", "marketplace"]),
            (CoreId::MarketplaceHttp, vec!["core", "plugins"]),
            (CoreId::MarketplaceHttp, vec!["core", "skills"]),
            (CoreId::McpHttp, vec!["plugins", "mcp"]),
        ]
    );
}

#[test]
fn the_caller_session_grant_is_listed_only_on_the_mcp_agent_and_issues_paths() {
    assert_eq!(
        rows_listing(KernelGrant::CallerSession),
        vec![
            (CoreId::AgentHttp, vec!["admin", "agents"]),
            (CoreId::AgentHttp, vec!["core", "contexts"]),
            (CoreId::AgentHttp, vec!["core", "artifacts"]),
            (CoreId::Issues, vec!["admin", "issues"]),
            (CoreId::McpHttp, vec!["plugins", "mcp"]),
        ]
    );
}

#[test]
fn the_session_context_grant_is_listed_only_on_core_contexts() {
    assert_eq!(
        rows_listing(KernelGrant::SessionContext),
        vec![(CoreId::AgentHttp, vec!["core", "contexts"])]
    );
}

#[test]
fn the_boot_profile_grant_is_listed_only_on_cloud() {
    assert_eq!(
        rows_listing(KernelGrant::BootProfile),
        vec![(CoreId::Cloud, vec!["cloud"])]
    );
}

fn forged_cloud(request: &'static [KernelGrant]) -> DeclaredCommands {
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: vec![AdmittedCommand {
            owner: ExtensionId::new(CoreId::Cloud.id()),
            core: Some(CoreId::Cloud),
            decl: CommandDecl::group::<CloudGroup>().requesting(request),
        }],
    })
}

#[test]
fn a_core_path_requesting_a_grant_its_row_does_not_list_is_refused() {
    const REQUESTS: [&[KernelGrant]; 5] = [
        &[KernelGrant::ServicesTree],
        &[KernelGrant::CallerSession],
        &[KernelGrant::SessionContext],
        &[KernelGrant::SigningKeys],
        &[KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity)],
    ];
    for request in REQUESTS {
        let grant = request[0];
        let error = CommandRegistry::build(&forged_cloud(request), &kernel_commands())
            .expect_err("the cloud row lists neither grant");
        assert!(
            matches!(error, RegistryError::GrantNotListed { grant: refused, .. } if refused == grant),
            "{error:?}"
        );
    }
}

#[test]
fn a_core_path_requesting_a_grant_its_row_lists_is_admitted() {
    CommandRegistry::build(
        &forged_cloud(&[KernelGrant::BootProfile]),
        &kernel_commands(),
    )
    .expect("negative control: the cloud row lists boot-profile");
}

const IDENTITY_GRANTS: [KernelGrant; 2] = [
    KernelGrant::SigningKeys,
    KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity),
];

#[test]
fn the_signing_keys_grant_is_listed_only_on_admin_keys_and_admin_bridge() {
    assert_eq!(
        rows_listing(KernelGrant::SigningKeys),
        vec![
            (CoreId::IdentityHttp, vec!["admin", "keys"]),
            (CoreId::IdentityHttp, vec!["admin", "bridge"]),
        ]
    );
}

#[test]
fn the_identity_plane_writer_grant_is_listed_only_on_admin_users() {
    assert_eq!(
        rows_listing(KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity)),
        vec![(CoreId::IdentityHttp, vec!["admin", "users"])]
    );
}

#[test]
fn every_granted_plane_is_a_kernel_grant() {
    for plane in GrantedPlane::ALL {
        assert!(KernelGrant::ALL.contains(&KernelGrant::ConfigPlaneWriter(plane)));
    }
}

fn forged_at(owner: &str, request: &'static [KernelGrant]) -> DeclaredCommands {
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: vec![AdmittedCommand {
            owner: ExtensionId::new(owner),
            core: CoreId::for_id(owner),
            decl: CommandDecl::group::<ProbeGroup>().requesting(request),
        }],
    })
}

#[test]
fn an_install_requesting_an_identity_grant_is_refused() {
    for grant in IDENTITY_GRANTS {
        let request: &'static [KernelGrant] = match grant {
            KernelGrant::SigningKeys => &[KernelGrant::SigningKeys],
            _ => &[KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity)],
        };
        let error =
            CommandRegistry::build(&forged_at("probe-install", request), &kernel_commands())
                .expect_err("an install id holds no kernel grant");
        assert!(
            matches!(error, RegistryError::GrantNotListed { grant: refused, .. } if refused == grant),
            "{grant}: {error:?}"
        );
    }
}

#[test]
fn the_identity_member_requesting_an_identity_grant_off_its_rows_is_refused() {
    let error = CommandRegistry::build(
        &forged_at(CoreId::IdentityHttp.id(), &[KernelGrant::SigningKeys]),
        &kernel_commands(),
    )
    .expect_err("the probe path is no identity row");
    assert!(matches!(
        error,
        RegistryError::GrantNotListed {
            grant: KernelGrant::SigningKeys,
            ..
        }
    ));
}
