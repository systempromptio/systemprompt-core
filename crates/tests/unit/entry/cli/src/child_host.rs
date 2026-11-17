#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::{CommandFactory, FromArgMatches};
use systemprompt_cli::args::{self, Cli};
use systemprompt_cli::kernel::{CommandRegistry, kernel_commands};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::cli::{CommandDescriptor, DataImpact, DescribeCommand, RoutingClass};

fn assert_child_host(desc: CommandDescriptor) {
    assert!(desc.child_host());
    assert!(
        desc.owns_signals(),
        "a child host installs no Ctrl-C handler"
    );
    assert!(
        !desc.serves_api(),
        "a child host is not stamped as the API server"
    );
    assert!(
        !desc.profile(),
        "a child host resolves no CLI profile or session"
    );
    assert!(!desc.secrets() && !desc.paths() && !desc.database());
    assert_eq!(desc.routing_class(), RoutingClass::LocalOnly);
    assert_eq!(desc.data_impact(), DataImpact::Preserving);
}

#[test]
fn the_child_host_descriptor_boots_nothing_from_the_cli() {
    assert_child_host(CommandDescriptor::CHILD_HOST);
}

#[test]
fn only_servers_and_child_hosts_own_their_signals() {
    assert!(CommandDescriptor::FULL.with_serves_api().owns_signals());
    assert!(!CommandDescriptor::FULL.owns_signals());
    assert!(!CommandDescriptor::PROFILE_ONLY.owns_signals());
    assert!(!CommandDescriptor::NONE.owns_signals());
}

#[test]
fn admin_agents_run_is_a_child_host() {
    let matches = Cli::command()
        .try_get_matches_from([
            "systemprompt",
            "admin",
            "agents",
            "run",
            "--agent-name",
            "child",
            "--port",
            "9000",
        ])
        .expect("admin agents run parses");
    let cli = Cli::from_arg_matches(&matches).expect("cli");
    let desc = cli.command.expect("a command").descriptor();
    assert_child_host(desc);
}

#[test]
fn admin_mcp_serve_is_a_kernel_command_and_a_child_host() {
    let registry = CommandRegistry::build(&DeclaredCommands::default(), &kernel_commands())
        .expect("the kernel groups build");
    assert!(
        !registry
            .entries()
            .iter()
            .any(|entry| entry.decl().path() == ["admin", "mcp"]),
        "admin mcp is part of the kernel's own admin tree, not a registry entry: {registry:?}"
    );

    let root = args::command(&registry).expect("assemble");
    let resolved = registry
        .resolve(
            &root
                .try_get_matches_from(["systemprompt", "admin", "mcp", "serve", "--server", "docs"])
                .expect("admin mcp serve parses"),
        )
        .expect("resolve");
    assert!(resolved.is_none(), "{resolved:?}");

    let matches = Cli::command()
        .try_get_matches_from(["systemprompt", "admin", "mcp", "serve", "--server", "docs"])
        .expect("admin mcp serve parses");
    let cli = Cli::from_arg_matches(&matches).expect("cli");
    assert_child_host(cli.command.expect("a command").descriptor());
}

#[test]
fn admin_mcp_serve_requires_a_server_id() {
    let registry = CommandRegistry::build(&DeclaredCommands::default(), &kernel_commands())
        .expect("the kernel groups build");
    let root = args::command(&registry).expect("assemble");
    assert!(
        root.try_get_matches_from(["systemprompt", "admin", "mcp", "serve"])
            .is_err()
    );
}
