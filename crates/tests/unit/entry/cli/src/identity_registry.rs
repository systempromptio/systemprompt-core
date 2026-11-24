//! The kernel command registry with the identity member's `admin users`,
//! `admin bridge` and `admin keys` groups admitted as `identity-http`
//! declares them, and a command line parsed and described through it as the
//! runner does.

use clap::{ArgMatches, CommandFactory, FromArgMatches};
use systemprompt_cli::args::{Cli, Commands};
use systemprompt_cli::kernel::{CommandRegistry, kernel_commands};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::admitted::{AdmittedCommand, DeclaredCommandParts};
use systemprompt_extension::cli::{
    CommandClaim, CommandDecl, CommandDescriptor, DescribeCommand, GrantedPlane, KernelGrant,
    parents,
};
use systemprompt_extension::reserved::CoreId;
use systemprompt_identifiers::ExtensionId;
use systemprompt_identity_entry::cli::bridge::BridgeCommands;
use systemprompt_identity_entry::cli::keys::KeysCommands;
use systemprompt_identity_entry::cli::users::UsersCommands;

pub fn registry() -> CommandRegistry {
    let decls = [
        CommandDecl::group_under::<parents::Admin, UsersCommands>()
            .requesting(&[KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity)])
            .claiming(&[CommandClaim::PreservesData, CommandClaim::DirectDatabase]),
        CommandDecl::group_under::<parents::Admin, BridgeCommands>()
            .requesting(&[KernelGrant::SigningKeys])
            .claiming(&[CommandClaim::PreservesData]),
        CommandDecl::group_under::<parents::Admin, KeysCommands>()
            .requesting(&[KernelGrant::SigningKeys])
            .claiming(&[CommandClaim::PreservesData]),
    ];
    let declared = DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: decls
            .into_iter()
            .map(|decl| AdmittedCommand {
                owner: ExtensionId::new("identity-http"),
                core: Some(CoreId::IdentityHttp),
                decl,
            })
            .collect(),
    });
    CommandRegistry::build(&declared, &kernel_commands()).expect("the identity registry builds")
}

pub fn parse(args: &[&str]) -> (CommandDescriptor, ArgMatches) {
    let registry = registry();
    let matches = registry
        .assemble(Cli::command())
        .expect("the command tree assembles")
        .try_get_matches_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"));
    let descriptor = match registry.resolve(&matches).expect("the line resolves") {
        Some(resolved) => resolved.parsed.descriptor(),
        None => Cli::from_arg_matches(&matches)
            .expect("typed parse")
            .command
            .as_ref()
            .map_or(CommandDescriptor::FULL, Commands::descriptor),
    };
    (descriptor, matches)
}
