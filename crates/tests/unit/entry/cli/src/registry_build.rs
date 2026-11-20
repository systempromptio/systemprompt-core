//! The command registry built from a declared-commands handoff and the kernel
//! groups: a contributed group parses and resolves with its own descriptor
//! and owner, and the registry refuses duplicate paths, a kernel shadow, a
//! missing parent, a leaf shadowing its parent's subcommand, an unlisted
//! grant, a descriptor holding an undeclared grant, and a reserved path under
//! a non-owner id.

use clap::Subcommand;
use systemprompt_cli::args;
use systemprompt_cli::kernel::{
    CommandOwner, CommandRegistry, RegistryError, ResolveError, kernel_commands,
};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::admitted::{AdmittedCommand, DeclaredCommandParts};
use systemprompt_extension::cli::{
    CliCommandGroup, CliCommandLeaf, CommandDecl, CommandDescriptor, CommandError, CommandHost,
    CommandOutput, CommandParent, DataImpact, DescribeCommand, KernelGrant, parents,
};
use systemprompt_extension::reserved::CoreId;
use systemprompt_identifiers::ExtensionId;

macro_rules! group {
    ($ty:ident, $name:literal, $descriptor:expr) => {
        #[derive(Debug, Subcommand)]
        enum $ty {
            Ping,
        }

        impl DescribeCommand for $ty {
            fn descriptor(&self) -> CommandDescriptor {
                $descriptor
            }
        }

        impl CliCommandGroup for $ty {
            const NAME: &'static str = $name;

            async fn run(self, _host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
                Err(CommandError::new(std::fmt::Error))
            }
        }
    };
}

group!(
    ProbeGroup,
    "probe",
    CommandDescriptor::FULL.with_data_impact(DataImpact::Destructive)
);
group!(AdminShadow, "admin", CommandDescriptor::NONE);
group!(McpImpostor, "mcp", CommandDescriptor::NONE);
group!(OrphanGroup, "orphan", CommandDescriptor::NONE);
group!(ChildHostGroup, "hosted", CommandDescriptor::CHILD_HOST);

macro_rules! leaf {
    ($ty:ident, $name:literal) => {
        #[derive(Debug, clap::Args)]
        struct $ty;

        impl DescribeCommand for $ty {
            fn descriptor(&self) -> CommandDescriptor {
                CommandDescriptor::PROFILE_ONLY
            }
        }

        impl CliCommandLeaf for $ty {
            const NAME: &'static str = $name;

            async fn run(self, _host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
                Err(CommandError::new(std::fmt::Error))
            }
        }
    };
}

leaf!(StatusLeaf, "status");
leaf!(ShadowLeaf, "ping");

struct UnderProbe;

impl CommandParent for UnderProbe {
    const PATH: &'static [&'static str] = &["probe"];
}

struct NoSuchParent;

impl CommandParent for NoSuchParent {
    const PATH: &'static [&'static str] = &["no-such-group"];
}

fn declared(commands: Vec<(&str, CommandDecl)>) -> DeclaredCommands {
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: commands
            .into_iter()
            .map(|(owner, decl)| AdmittedCommand {
                owner: ExtensionId::new(owner),
                core: None,
                decl,
            })
            .collect(),
    })
}

fn registry(commands: Vec<(&str, CommandDecl)>) -> CommandRegistry {
    CommandRegistry::build(&declared(commands), &kernel_commands()).expect("the registry builds")
}

#[test]
fn a_contributed_group_resolves_with_its_descriptor_and_owner() {
    let registry = registry(vec![("probe-ext", CommandDecl::group::<ProbeGroup>())]);
    let root = args::command(&registry).expect("the tree assembles");
    let resolved = registry
        .resolve(
            &root
                .try_get_matches_from(["systemprompt", "probe", "ping"])
                .expect("parses"),
        )
        .expect("resolves")
        .expect("a contributed command");
    assert_eq!(resolved.path, ["probe"]);
    assert_eq!(
        resolved.owner,
        CommandOwner::Member(ExtensionId::new("probe-ext"))
    );
    assert_eq!(
        resolved.parsed.descriptor().data_impact(),
        DataImpact::Destructive
    );
}

#[test]
fn a_kernel_group_resolves_to_the_kernel() {
    let registry = registry(vec![]);
    let root = args::command(&registry).expect("the tree assembles");
    let resolved = registry
        .resolve(
            &root
                .try_get_matches_from(["systemprompt", "build", "mcp"])
                .expect("parses"),
        )
        .expect("resolves")
        .expect("build is a registry entry");
    assert_eq!(resolved.owner, CommandOwner::Kernel);
}

#[test]
fn two_owners_of_one_path_are_refused() {
    let error = CommandRegistry::build(
        &declared(vec![
            ("one", CommandDecl::group::<ProbeGroup>()),
            ("two", CommandDecl::group::<ProbeGroup>()),
        ]),
        &kernel_commands(),
    )
    .expect_err("one path, two owners");
    assert!(matches!(error, RegistryError::Duplicate { .. }));
}

#[test]
fn a_group_shadowing_a_kernel_command_is_refused() {
    let registry = registry(vec![("probe-ext", CommandDecl::group::<AdminShadow>())]);
    assert!(matches!(
        args::command(&registry),
        Err(RegistryError::ShadowsKernel { .. })
    ));
}

#[test]
fn a_group_under_a_missing_parent_is_refused() {
    let registry = registry(vec![(
        "probe-ext",
        CommandDecl::group_under::<NoSuchParent, OrphanGroup>(),
    )]);
    assert!(matches!(
        args::command(&registry),
        Err(RegistryError::UnknownParent { .. })
    ));
}

#[test]
fn a_leaf_under_a_contributed_group_resolves_and_may_not_shadow_its_subcommand() {
    let resolving = registry(vec![
        ("probe-ext", CommandDecl::group::<ProbeGroup>()),
        (
            "probe-ext",
            CommandDecl::leaf_under::<UnderProbe, StatusLeaf>(),
        ),
    ]);
    let root = args::command(&resolving).expect("the tree assembles");
    let resolved = resolving
        .resolve(
            &root
                .try_get_matches_from(["systemprompt", "probe", "status"])
                .expect("parses"),
        )
        .expect("resolves")
        .expect("the leaf is an entry");
    assert_eq!(resolved.path, ["probe", "status"]);

    let shadowing = registry(vec![
        ("probe-ext", CommandDecl::group::<ProbeGroup>()),
        (
            "probe-ext",
            CommandDecl::leaf_under::<UnderProbe, ShadowLeaf>(),
        ),
    ]);
    assert!(matches!(
        args::command(&shadowing),
        Err(RegistryError::ShadowsGroup { .. })
    ));
}

#[test]
fn a_grant_unlisted_for_the_path_is_refused_at_build() {
    let decl = CommandDecl::group::<ProbeGroup>().requesting(&[KernelGrant::ChildHost]);
    let error = CommandRegistry::build(&declared(vec![("probe-ext", decl)]), &kernel_commands())
        .expect_err("child host is listed only at admin mcp");
    assert!(matches!(
        error,
        RegistryError::GrantNotListed {
            grant: KernelGrant::ChildHost,
            ..
        }
    ));
}

#[test]
fn a_descriptor_holding_an_undeclared_grant_is_refused_at_resolution() {
    let registry = registry(vec![("probe-ext", CommandDecl::group::<ChildHostGroup>())]);
    let root = args::command(&registry).expect("the tree assembles");
    let error = registry
        .resolve(
            &root
                .try_get_matches_from(["systemprompt", "hosted", "ping"])
                .expect("parses"),
        )
        .expect_err("the descriptor holds child host without requesting it");
    assert!(matches!(
        error,
        ResolveError::Registry(RegistryError::UndeclaredGrant {
            grant: KernelGrant::ChildHost,
            ..
        })
    ));
}

#[test]
fn a_member_at_the_kernels_reserved_path_is_refused() {
    let decl = CommandDecl::group_under::<parents::Admin, McpImpostor>()
        .requesting(&[KernelGrant::ChildHost]);
    let error = CommandRegistry::build(&declared(vec![("probe-ext", decl)]), &[])
        .expect_err("admin mcp is the kernel's");
    assert!(matches!(
        error,
        RegistryError::ReservedPath {
            owner: CoreId::Kernel,
            ..
        }
    ));
}
