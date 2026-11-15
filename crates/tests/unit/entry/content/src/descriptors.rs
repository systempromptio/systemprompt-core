//! The content commands as the CLI router classifies them: `core content` and
//! `web sitemap` register under their kernel parents, deleting content, a
//! source or a link is destructive, and the read commands preserve the data
//! behind the profile.

use systemprompt_cli::kernel::{
    CommandDescriptor, CommandRegistry, DataImpact, describe_command_line,
};

fn describe(args: &[&str]) -> CommandDescriptor {
    let argv: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    describe_command_line(&argv).unwrap_or_else(|e| panic!("describe {args:?}: {e:#}"))
}

#[test]
fn the_content_commands_register_under_core_and_web() {
    let registry = CommandRegistry::collected().expect("registrations are consistent");
    let paths: Vec<String> = registry
        .registrations()
        .iter()
        .map(|registration| registration.path())
        .collect();
    assert!(paths.iter().any(|p| p == "core content"), "{paths:?}");
    assert!(paths.iter().any(|p| p == "web sitemap"), "{paths:?}");
}

#[test]
fn deleting_content_sources_or_links_is_destructive() {
    for args in [
        ["core", "content", "delete", "c", "--yes"].as_slice(),
        &["core", "content", "delete-source", "s", "--yes"],
        &["core", "content", "link", "delete", "l", "--yes"],
    ] {
        let desc = describe(args);
        assert!(desc.is_destructive(), "{args:?}");
        assert_eq!(desc.data_impact(), DataImpact::Destructive, "{args:?}");
    }
}

#[test]
fn the_read_commands_preserve_data() {
    for args in [
        ["core", "content", "list"].as_slice(),
        &["core", "content", "link", "list"],
        &["core", "content", "analytics", "journey"],
        &["web", "sitemap", "show"],
    ] {
        let desc = describe(args);
        assert!(!desc.is_destructive(), "{args:?}");
        assert_eq!(desc.data_impact(), DataImpact::Preserving, "{args:?}");
    }
}

#[test]
fn core_content_needs_the_full_context_and_sitemap_the_profile_paths() {
    let content = describe(&["core", "content", "list"]);
    assert!(content.database(), "core content reads the database");
    let sitemap = describe(&["web", "sitemap", "show"]);
    assert!(sitemap.paths(), "web sitemap reads the profile paths");
    assert!(!sitemap.database(), "web sitemap never dials the database");
}
