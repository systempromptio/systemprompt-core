//! The `analytics` group and the `core content popular` leaf reach the CLI
//! through the kernel registry, with the bootstrap they had as kernel
//! commands: full bootstrap, read-only routing, data preserved, a local
//! `--export` for `analytics`, direct `--database-url` routing, and the
//! `analytics` `--help` slot between `admin` and `web`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::kernel::{
    CommandRegistry, DataImpact, DbUrlRouting, RoutingClass, describe_command_line,
};

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
    assert!(paths.iter().any(|p| p == "core content popular"), "{paths:?}");
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
        assert_eq!(descriptor.routing_class(), RoutingClass::ReadOnly, "{args:?}");
        assert_eq!(descriptor.data_impact(), DataImpact::Preserving, "{args:?}");
        assert!(!descriptor.cloud_operator(), "{args:?}");
        assert_eq!(resolved_routing(args), DbUrlRouting::Direct, "{args:?}");
    }
}

#[test]
fn popular_is_a_leaf_under_core_content_that_preserves_data() {
    let args = ["core", "content", "popular", "blog"];
    let descriptor = describe_command_line(&argv(&args)).expect("parses");
    assert!(descriptor.database());
    assert!(!descriptor.local_export());
    assert_eq!(descriptor.data_impact(), DataImpact::Preserving);
    assert_eq!(resolved_routing(&args), DbUrlRouting::Direct);
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
