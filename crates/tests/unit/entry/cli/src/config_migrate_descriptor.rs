//! `admin config migrate` must bootstrap without loading the services tree.
//!
//! The paths phase loads `services/` through the catalog loader, which
//! refuses a pre-0.73 tree with the hint to run `config migrate`. Migrate
//! rewrites exactly that tree, so without `--data` it may load the profile
//! and secrets only. `--data` runs over an already-migrated tree and reads
//! its database URL from it, and every other `admin config` command keeps
//! loading the tree, which is what refuses a legacy one.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::args::Commands;
use systemprompt_cli::descriptor::DescribeCommand;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: Commands,
}

fn descriptor(args: &[&str]) -> systemprompt_cli::descriptor::CommandDescriptor {
    Harness::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("failed to parse {args:?}: {e}"))
        .cmd
        .descriptor()
}

#[test]
fn migrate_loads_the_profile_and_secrets_but_not_the_services_tree() {
    for args in [
        vec!["admin", "config", "migrate"],
        vec!["admin", "config", "migrate", "--check"],
        vec!["admin", "config", "migrate", "--services", "/tmp/services"],
    ] {
        let desc = descriptor(&args);
        assert!(desc.profile(), "{args:?} needs the profile");
        assert!(desc.secrets(), "{args:?} needs the secrets");
        assert!(
            !desc.paths(),
            "{args:?} must not load the legacy tree it rewrites"
        );
        assert!(!desc.database(), "{args:?} touches files only");
    }
}

#[test]
fn migrate_data_loads_the_migrated_tree() {
    assert!(descriptor(&["admin", "config", "migrate", "--data"]).paths());
}

#[test]
fn other_config_commands_still_load_the_tree_that_refuses_a_legacy_shape() {
    for args in [
        vec!["admin", "config", "list"],
        vec!["admin", "config", "validate"],
    ] {
        assert!(descriptor(&args).paths(), "{args:?} must load services/");
    }
}
