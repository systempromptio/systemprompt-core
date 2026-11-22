//! A command's routing class, data impact and `--database-url` routing are
//! projected from its descriptor and declaration, and a member holds a
//! relaxing claim only where its reserved CLI path's row confers it (A10):
//! the composition refuses an install declaring a claim, the CLI registry
//! refuses a forged handoff whose declaration lists a claim its path's row does
//! not confer, and resolution refuses a descriptor that makes a claim its
//! declaration does not list. An install command that touches the database is
//! remote-mutating and destructive.

use systemprompt::CompositionError;
use systemprompt_cli::kernel::{CommandRegistry, RegistryError, ResolveError, kernel_commands};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::admitted::{AdmittedCommand, DeclaredCommandParts};
use systemprompt_extension::cli::{
    CliCommandGroup, CommandClaim, CommandDecl, CommandDescriptor, CommandError, CommandHost,
    CommandOutput, DataImpact, DbUrlRouting, DescribeCommand, RoutingClass,
};
use systemprompt_extension::reserved::{CoreId, RESERVED_CLI_PATHS};
use systemprompt_identifiers::ExtensionId;

use crate::cli_probe::AnalyticsGroup;
use crate::probe::{Probe, build_with};

macro_rules! described_group {
    ($ty:ident, $name:literal, $descriptor:expr) => {
        #[derive(Debug, Clone, clap::Subcommand)]
        enum $ty {
            Status,
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

described_group!(
    ReadOnlyGroup,
    "probe-read",
    CommandDescriptor::FULL
        .with_read_only()
        .with_data_impact(DataImpact::Destructive)
);
described_group!(
    LocalGroup,
    "probe-local",
    CommandDescriptor::LOCAL_DATABASE.with_data_impact(DataImpact::Destructive)
);
described_group!(PreservingGroup, "probe-preserving", CommandDescriptor::FULL);
described_group!(
    MutatingGroup,
    "probe-mutating",
    CommandDescriptor::FULL.with_data_impact(DataImpact::Destructive)
);
described_group!(
    OfflineGroup,
    "probe-offline",
    CommandDescriptor::PROFILE_ONLY
);

fn forged(owner: &str, core: Option<CoreId>, decl: CommandDecl) -> DeclaredCommands {
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: vec![AdmittedCommand {
            owner: ExtensionId::new(owner),
            core,
            decl,
        }],
    })
}

fn resolve(
    registry: &CommandRegistry,
    group: &str,
) -> Result<Option<systemprompt_cli::kernel::ResolvedCommand>, ResolveError> {
    let matches = systemprompt_cli::args::command(registry)
        .expect("the probe group attaches")
        .try_get_matches_from(["systemprompt", group, "status"])
        .expect("the probe command parses");
    registry.resolve(&matches)
}

fn install(decl: CommandDecl) -> CommandRegistry {
    CommandRegistry::build(&forged("probe-install", None, decl), &kernel_commands())
        .expect("an install group that lists no claim is registered")
}

#[test]
fn the_composition_refuses_an_install_declaring_any_claim() {
    for claim in CommandClaim::ALL {
        let claims: &'static [CommandClaim] = match claim {
            CommandClaim::ReadOnlyRouting => &[CommandClaim::ReadOnlyRouting],
            CommandClaim::LocalDatabase => &[CommandClaim::LocalDatabase],
            CommandClaim::PreservesData => &[CommandClaim::PreservesData],
            CommandClaim::DirectDatabase => &[CommandClaim::DirectDatabase],
        };
        let error = build_with(Probe::new("probe-install", move |c| {
            c.command(CommandDecl::group::<MutatingGroup>().claiming(claims));
        }))
        .expect_err("an install command holds no claim");
        assert!(
            matches!(error, CompositionError::ClaimNotConferred { claim: refused, .. } if refused == claim),
            "{error:?}"
        );
    }
    build_with(Probe::new("probe-install", |c| {
        c.command(CommandDecl::group::<MutatingGroup>());
    }))
    .expect("negative control: an install command claiming nothing is admitted");
}

#[test]
fn the_registry_refuses_a_forged_claim_its_path_does_not_confer() {
    let error = CommandRegistry::build(
        &forged(
            "probe-install",
            None,
            CommandDecl::group::<MutatingGroup>().claiming(&[CommandClaim::DirectDatabase]),
        ),
        &kernel_commands(),
    )
    .expect_err("an unreserved path confers no claim");
    assert!(matches!(
        error,
        RegistryError::ClaimNotListed {
            claim: CommandClaim::DirectDatabase,
            ..
        }
    ));
    let error = CommandRegistry::build(
        &forged(
            CoreId::AnalyticsHttp.id(),
            Some(CoreId::AnalyticsHttp),
            CommandDecl::group::<AnalyticsGroup>().claiming(&[CommandClaim::LocalDatabase]),
        ),
        &kernel_commands(),
    )
    .expect_err("the analytics row confers no local-database claim");
    assert!(matches!(
        error,
        RegistryError::ClaimNotListed {
            claim: CommandClaim::LocalDatabase,
            ..
        }
    ));
    CommandRegistry::build(
        &forged(
            CoreId::AnalyticsHttp.id(),
            Some(CoreId::AnalyticsHttp),
            CommandDecl::group::<AnalyticsGroup>().claiming(&[CommandClaim::DirectDatabase]),
        ),
        &kernel_commands(),
    )
    .expect("negative control: the analytics row confers direct-database");
}

#[test]
fn resolution_refuses_a_descriptor_claim_the_declaration_does_not_list() {
    for (decl, group, claim) in [
        (
            CommandDecl::group::<ReadOnlyGroup>(),
            "probe-read",
            CommandClaim::ReadOnlyRouting,
        ),
        (
            CommandDecl::group::<LocalGroup>(),
            "probe-local",
            CommandClaim::LocalDatabase,
        ),
        (
            CommandDecl::group::<PreservingGroup>(),
            "probe-preserving",
            CommandClaim::PreservesData,
        ),
    ] {
        let error = resolve(&install(decl), group).expect_err("the descriptor makes a claim");
        assert!(
            matches!(
                &error,
                ResolveError::Registry(RegistryError::UndeclaredClaim { claim: refused, .. })
                    if *refused == claim
            ),
            "{group}: {error:?}"
        );
    }
}

#[test]
fn an_install_database_command_is_remote_mutating_destructive_and_profile_bound() {
    let registry = install(CommandDecl::group::<MutatingGroup>());
    let resolved = resolve(&registry, "probe-mutating")
        .expect("negative control: a claimless descriptor resolves")
        .expect("the probe group is a contributed command");
    let descriptor = resolved.parsed.descriptor();
    assert_eq!(descriptor.routing_class(), RoutingClass::Mutating);
    assert!(descriptor.data_impact().destroys_data());
    assert_eq!(resolved.parsed.db_url_routing(), DbUrlRouting::Unsupported);

    let offline = install(CommandDecl::group::<OfflineGroup>());
    let resolved = resolve(&offline, "probe-offline")
        .expect("a command without a database makes no claim")
        .expect("the probe group is a contributed command");
    assert_eq!(resolved.parsed.descriptor().routing_class(), RoutingClass::LocalOnly);
}

#[test]
fn direct_database_routing_is_projected_from_the_declaration() {
    let registry = CommandRegistry::build(
        &forged(
            CoreId::AnalyticsHttp.id(),
            Some(CoreId::AnalyticsHttp),
            CommandDecl::group::<AnalyticsGroup>().claiming(&[CommandClaim::DirectDatabase]),
        ),
        &kernel_commands(),
    )
    .expect("the analytics row confers direct-database");
    let resolved = resolve(&registry, "analytics")
        .expect("the analytics probe resolves")
        .expect("the analytics probe is a contributed command");
    assert_eq!(resolved.parsed.db_url_routing(), DbUrlRouting::Direct);

    let registry = CommandRegistry::build(
        &forged(
            CoreId::AnalyticsHttp.id(),
            Some(CoreId::AnalyticsHttp),
            CommandDecl::group::<AnalyticsGroup>(),
        ),
        &kernel_commands(),
    )
    .expect("the analytics path without claims");
    let resolved = resolve(&registry, "analytics")
        .expect("the analytics probe resolves")
        .expect("the analytics probe is a contributed command");
    assert_eq!(resolved.parsed.db_url_routing(), DbUrlRouting::Unsupported);
}

#[test]
fn the_direct_database_claim_is_listed_only_on_the_bare_url_paths() {
    let direct: Vec<(CoreId, Vec<&str>)> = RESERVED_CLI_PATHS
        .iter()
        .filter(|row| row.claims.contains(&CommandClaim::DirectDatabase))
        .map(|row| (row.owner, row.path.to_vec()))
        .collect();
    assert_eq!(
        direct,
        vec![
            (CoreId::AnalyticsHttp, vec!["analytics"]),
            (CoreId::AnalyticsHttp, vec!["core", "content", "popular"]),
            (CoreId::ContentHttp, vec!["core", "content"]),
            (CoreId::FilesHttp, vec!["core", "files"]),
        ]
    );
}
