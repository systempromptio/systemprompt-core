//! The reference composition admits `marketplace-http`'s commands as its
//! conferral describes: the `core marketplace`, `core plugins` and
//! `core skills` groups, each requesting only the services-tree grant, and a
//! requirement on the owner-bound `Arc<OrganizationSkillResolver>` the
//! `managed_resources` member provides. `core plugins` and
//! `core marketplace explain` need no database; `core skills` does. A
//! profile disabling `marketplace-http` refuses each group at dispatch; one
//! disabling `managed_resources` refuses boot. A forged install claiming the
//! `core skills` path, or the services-tree grant on its own path, is
//! refused by the registry.

use std::sync::Arc;

use systemprompt::{CompositionError, reference};
use systemprompt_cli::CliRunError;
use systemprompt_cli::kernel::{
    CommandRegistry, RegistryError, ResolvedCommand, describe_command_line, kernel_commands,
};
use systemprompt_cli::runner::refuse_disabled;
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::admitted::{AdmittedCommand, DeclaredCommandParts};
use systemprompt_extension::cli::{
    CommandDecl, CommandDescriptor, DataImpact, KernelGrant, parents,
};
use systemprompt_extension::reserved::CoreId;
use systemprompt_identifiers::ExtensionId;

use crate::cli_probe::{ProbeGroup, SkillsGroup};
use crate::probe::{build, profile};

const OWNER: &str = "marketplace-http";
const GROUPS: [&str; 3] = ["marketplace", "plugins", "skills"];

fn registry() -> CommandRegistry {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands")
}

fn describe(args: &[&str]) -> CommandDescriptor {
    let argv: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    describe_command_line(&registry(), &argv).unwrap_or_else(|e| panic!("{args:?}: {e}"))
}

fn resolved(args: &[&str]) -> ResolvedCommand {
    let registry = registry();
    let matches = systemprompt_cli::args::command(&registry)
        .expect("the reference commands attach")
        .try_get_matches_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .expect("the command parses");
    registry
        .resolve(&matches)
        .expect("the command resolves")
        .expect("the command is a contributed command")
}

fn forged(owner: &str, decl: CommandDecl) -> DeclaredCommands {
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: vec![AdmittedCommand {
            owner: ExtensionId::new(owner),
            core: CoreId::for_id(owner),
            decl,
        }],
    })
}

fn skills_decl(grants: &'static [KernelGrant]) -> CommandDecl {
    CommandDecl::group_under::<parents::Core, SkillsGroup>().requesting(grants)
}

#[test]
fn marketplace_http_declares_the_three_core_groups_with_only_the_services_tree_grant() {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    let commands = declared.commands();
    let owned: Vec<(Vec<&str>, Vec<KernelGrant>)> = commands
        .commands()
        .iter()
        .filter(|command| command.owner.as_str() == OWNER)
        .map(|command| (command.decl.path(), command.decl.grants().to_vec()))
        .collect();
    let expected: Vec<(Vec<&str>, Vec<KernelGrant>)> = GROUPS
        .iter()
        .map(|group| (vec!["core", *group], vec![KernelGrant::ServicesTree]))
        .collect();
    assert_eq!(owned, expected);
}

#[test]
fn marketplace_http_requires_the_organization_skill_resolver() {
    let admitted = build(reference()).expect("the reference composition admits");
    let member = admitted
        .built()
        .members()
        .find(|member| member.owner.as_str() == OWNER)
        .expect("marketplace-http is admitted");
    let required: Vec<&str> = member
        .sink
        .requires()
        .iter()
        .map(|requirement| requirement.type_name())
        .collect();
    assert!(
        required.contains(&std::any::type_name::<
            Arc<systemprompt::marketplace::managed::OrganizationSkillResolver>,
        >()),
        "{required:?}"
    );
}

#[test]
fn the_offline_commands_need_no_database_and_skills_does() {
    for args in [
        ["core", "plugins", "list"].as_slice(),
        &["core", "plugins", "validate"],
        &["core", "plugins", "generate"],
        &["core", "marketplace", "explain"],
    ] {
        let descriptor = describe(args);
        assert!(!descriptor.database(), "{args:?}");
        assert!(descriptor.paths(), "{args:?}");
        assert!(descriptor.holds(KernelGrant::ServicesTree), "{args:?}");
        assert_eq!(descriptor.data_impact(), DataImpact::Preserving, "{args:?}");
    }
    let import = describe(&[
        "core",
        "marketplace",
        "import",
        "--from",
        "a",
        "--into",
        "b",
    ]);
    assert!(!import.profile() && !import.database());
    for args in [
        ["core", "skills", "list"].as_slice(),
        &["core", "skills", "show", "s"],
    ] {
        let descriptor = describe(args);
        assert!(descriptor.database(), "{args:?}");
        assert!(descriptor.holds(KernelGrant::ServicesTree), "{args:?}");
        assert_eq!(descriptor.data_impact(), DataImpact::Preserving, "{args:?}");
    }
}

#[test]
fn a_profile_disabling_marketplace_http_refuses_each_group_at_dispatch() {
    for args in [
        ["core", "marketplace", "explain"].as_slice(),
        &["core", "plugins", "list"],
        &["core", "skills", "list"],
    ] {
        let command = resolved(args);
        let error = refuse_disabled(&command, &profile(&[OWNER]))
            .expect_err("the profile disables the command's owner");
        assert!(
            matches!(
                &error,
                CliRunError::CommandDisabled { path, owner }
                    if path[..2] == args[..2] && owner.as_str() == OWNER
            ),
            "{args:?}: {error:?}"
        );
        assert!(
            refuse_disabled(&command, &profile(&[])).is_ok(),
            "negative control: {args:?}"
        );
    }
}

#[test]
fn a_profile_disabling_managed_resources_refuses_boot() {
    let error = reference()
        .build(&profile(&["managed_resources"]))
        .expect_err("managed_resources is a required member");
    assert!(matches!(
        &error,
        CompositionError::RequiredDisabled(id) if id.as_str() == "managed_resources"
    ));
}

#[test]
fn a_forged_install_claiming_the_core_skills_path_is_refused() {
    for grants in [&[][..], &[KernelGrant::ServicesTree][..]] {
        let error = CommandRegistry::build(
            &forged("probe-install", skills_decl(grants)),
            &kernel_commands(),
        )
        .expect_err("an install id cannot hold the core skills path");
        assert!(
            matches!(
                error,
                RegistryError::ReservedPath {
                    owner: CoreId::MarketplaceHttp,
                    ..
                }
            ),
            "{grants:?}: {error:?}"
        );
    }
    CommandRegistry::build(
        &forged(OWNER, skills_decl(&[KernelGrant::ServicesTree])),
        &kernel_commands(),
    )
    .expect("negative control: marketplace-http holds the core skills path");
}

#[test]
fn a_forged_install_requesting_the_services_tree_grant_is_refused() {
    let decl = CommandDecl::group::<ProbeGroup>().requesting(&[KernelGrant::ServicesTree]);
    let error = CommandRegistry::build(&forged("probe-install", decl), &kernel_commands())
        .expect_err("the services-tree grant is listed only on the marketplace paths");
    assert!(
        matches!(
            error,
            RegistryError::GrantNotListed {
                grant: KernelGrant::ServicesTree,
                ..
            }
        ),
        "{error:?}"
    );
}
