//! The `analytics` group and the `core content popular` leaf reach the CLI
//! through the kernel registry, with the bootstrap they had as kernel
//! commands: full bootstrap, read-only routing, data preserved, a local
//! `--export` for `analytics`, direct `--database-url` routing, and the
//! `analytics` `--help` slot between `admin` and `web`. The content entry
//! crate is linked so `core content popular` is driven through the real
//! assembled tree, where `core content` is itself a registered group.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::kernel::{
    CommandRegistry, DataImpact, DbUrlRouting, RoutingClass, describe_command_line,
};
use systemprompt_content_entry as _;

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

fn resolved_routing(args: &[&str]) -> DbUrlRouting {
    let registry = CommandRegistry::collected().expect("collected registrations are disjoint");
    let root = systemprompt_cli::args::command(&registry).expect("assemble");
    let matches = root
        .try_get_matches_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"));
    registry
        .resolve(&matches)
        .expect("resolve")
        .unwrap_or_else(|| panic!("{args:?} resolves to a registration"))
        .db_url_routing()
}

#[test]
fn both_commands_are_collected_from_their_registrations() {
    let registry = CommandRegistry::collected().expect("collected registrations are disjoint");
    let paths: Vec<String> = registry
        .registrations()
        .iter()
        .map(|entry| entry.path().to_owned())
        .collect();
    assert!(paths.iter().any(|p| p == "analytics"), "{paths:?}");
    assert!(
        paths.iter().any(|p| p == "core content popular"),
        "{paths:?}"
    );
}

#[test]
fn every_analytics_command_is_a_read_only_local_export_that_preserves_data() {
    for args in [
        &["analytics", "overview"][..],
        &["analytics", "traffic", "pages"],
        &["analytics", "costs"],
        &["analytics", "sessions", "live", "--no-refresh"],
    ] {
        let descriptor = describe_command_line(&argv(args)).expect("parses");
        assert!(descriptor.local_export(), "{args:?}");
        assert!(descriptor.database(), "{args:?}");
        assert!(descriptor.skip_validation(), "{args:?}");
        assert_eq!(
            descriptor.routing_class(),
            RoutingClass::ReadOnly,
            "{args:?}"
        );
        assert_eq!(descriptor.data_impact(), DataImpact::Preserving, "{args:?}");
        assert!(!descriptor.cloud_operator(), "{args:?}");
        assert_eq!(resolved_routing(args), DbUrlRouting::Direct, "{args:?}");
    }
}

fn assembled() -> clap::Command {
    let registry = CommandRegistry::collected().expect("collected registrations are disjoint");
    systemprompt_cli::args::command(&registry).expect("assemble")
}

#[test]
fn core_content_is_the_registered_group_popular_attaches_under() {
    let registry = CommandRegistry::collected().expect("collected");
    let group = |path: &str| {
        registry
            .registrations()
            .iter()
            .find(|entry| entry.path() == path)
            .unwrap_or_else(|| panic!("`{path}` is registered: {registry:?}"))
            .group()
    };
    assert!(group("core content").ends_with("ContentCommands"));
    assert!(group("core content popular").ends_with("PopularArgs"));
}

#[test]
fn popular_help_parses_through_the_assembled_tree() {
    let err = assembled()
        .try_get_matches_from(["systemprompt", "core", "content", "popular", "--help"])
        .unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp, "{err}");
    let help = err.to_string();
    assert!(help.contains("--since"), "{help}");
    assert!(help.contains("--limit"), "{help}");
}

#[test]
fn popular_is_listed_after_the_content_groups_own_subcommands() {
    let mut root = assembled();
    let help = root
        .find_subcommand_mut("core")
        .and_then(|core| core.find_subcommand_mut("content"))
        .expect("core content is attached")
        .render_help()
        .to_string();
    let position = |name: &str| {
        help.find(&format!("\n  {name} "))
            .unwrap_or_else(|| panic!("`{name}` is listed in --help:\n{help}"))
    };
    assert!(position("list") < position("files"));
    assert!(position("files") < position("popular"));
}

#[test]
fn popular_resolves_to_its_own_leaf_descriptor() {
    let args = ["core", "content", "popular", "blog", "--since", "7d"];
    let descriptor = describe_command_line(&argv(&args)).expect("parses");
    assert!(descriptor.profile() && descriptor.secrets() && descriptor.database());
    assert_eq!(descriptor.routing_class(), RoutingClass::Mutating);
    assert_eq!(descriptor.data_impact(), DataImpact::Preserving);
    assert!(!descriptor.local_export());
    assert!(!descriptor.skip_validation());
    assert!(!descriptor.cloud_operator());
    assert_eq!(resolved_routing(&args), DbUrlRouting::Direct);
}

#[test]
fn the_content_groups_own_subcommands_keep_the_groups_descriptor() {
    let list = describe_command_line(&argv(&["core", "content", "list"])).expect("parses");
    assert!(list.database());
    assert_eq!(list.data_impact(), DataImpact::Preserving);
    assert_eq!(resolved_routing(&["core", "content", "list"]), DbUrlRouting::Direct);

    let delete =
        describe_command_line(&argv(&["core", "content", "delete", "c1"])).expect("parses");
    assert_eq!(delete.data_impact(), DataImpact::Destructive);
}

#[test]
fn the_analytics_group_without_a_subcommand_is_refused() {
    assert!(describe_command_line(&argv(&["analytics"])).is_err());
}

#[test]
fn help_lists_analytics_in_its_slot() {
    let registry = CommandRegistry::collected().expect("collected");
    let help = systemprompt_cli::args::command(&registry)
        .expect("assemble")
        .render_help()
        .to_string();
    let position = |name: &str| {
        help.find(&format!("\n  {name} "))
            .unwrap_or_else(|| panic!("`{name}` is listed in --help:\n{help}"))
    };
    assert!(position("admin") < position("analytics"));
    assert!(position("analytics") < position("web"));
}
