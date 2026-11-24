use clap::{ArgMatches, FromArgMatches};
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

fn stamped(decl: CommandDecl) -> AdmittedCommand {
    AdmittedCommand {
        owner: ExtensionId::new(CoreId::IdentityHttp.id()),
        core: Some(CoreId::IdentityHttp),
        decl,
    }
}

pub fn declared() -> DeclaredCommands {
    let preserving: &'static [CommandClaim] = &[CommandClaim::PreservesData];
    DeclaredCommands::__from_facade(DeclaredCommandParts {
        commands: vec![
            stamped(
                CommandDecl::group_under::<parents::Admin, UsersCommands>()
                    .requesting(&[KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity)])
                    .claiming(&[CommandClaim::DirectDatabase, CommandClaim::PreservesData]),
            ),
            stamped(
                CommandDecl::group_under::<parents::Admin, KeysCommands>()
                    .requesting(&[KernelGrant::SigningKeys])
                    .claiming(preserving),
            ),
            stamped(
                CommandDecl::group_under::<parents::Admin, BridgeCommands>()
                    .requesting(&[KernelGrant::SigningKeys])
                    .claiming(preserving),
            ),
        ],
    })
}

pub fn registry() -> CommandRegistry {
    CommandRegistry::build(&declared(), &kernel_commands()).expect("the identity groups register")
}

pub fn matches(args: &[&str]) -> ArgMatches {
    systemprompt_cli::args::command(&registry())
        .expect("the identity groups attach")
        .try_get_matches_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
}

pub fn descriptor_of(matches: &ArgMatches) -> CommandDescriptor {
    if let Some(resolved) = registry().resolve(matches).expect("the command resolves") {
        return resolved.parsed.descriptor();
    }
    Cli::from_arg_matches(matches)
        .expect("typed parse")
        .command
        .as_ref()
        .map_or(CommandDescriptor::FULL, Commands::descriptor)
}

pub fn descriptor(args: &[&str]) -> CommandDescriptor {
    descriptor_of(&matches(args))
}
