#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::{CommandFactory, FromArgMatches};
use systemprompt_cli::args::{self, Cli};
use systemprompt_cli::descriptor::{
    CommandDescriptor, DataImpact, DbUrlRouting, DescribeCommand, RoutingClass,
};
use systemprompt_cli::kernel::CommandRegistry;

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
fn admin_mcp_serve_is_registered_through_the_seam_as_a_child_host() {
    let registry = CommandRegistry::collected().expect("collected registrations are disjoint");
    assert!(
        registry
            .registrations()
            .iter()
            .any(|entry| entry.path() == "admin mcp" && entry.group().ends_with("McpHostCommands")),
        "{registry:?}"
    );

    let root = args::command(&registry).expect("assemble");
    let serve = registry
        .resolve(
            &root
                .try_get_matches_from(["systemprompt", "admin", "mcp", "serve", "--server", "docs"])
                .expect("admin mcp serve parses"),
        )
        .expect("resolve")
        .expect("admin mcp serve resolves to its registration");
    assert_child_host(serve.descriptor());
    assert_eq!(serve.db_url_routing(), DbUrlRouting::Unsupported);
}

#[test]
fn admin_mcp_serve_requires_a_server_id() {
    let registry = CommandRegistry::collected().expect("collected registrations are disjoint");
    let root = args::command(&registry).expect("assemble");
    assert!(
        root.try_get_matches_from(["systemprompt", "admin", "mcp", "serve"])
            .is_err()
    );
}
