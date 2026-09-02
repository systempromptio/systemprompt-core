//! The env-binding help surface: clap interpolates the *live* value of an
//! env-bound argument into `--help` unless `hide_env_values` is set.
//!
//! A prod gateway scan caught a real Google API key that reached a model this
//! way — an agent ran `admin setup --help` inside a process whose environment
//! our own `Secrets::to_subprocess_env` had populated. These walk the built
//! command tree rather than a hand-written list, so an argument added later
//! cannot reintroduce the leak silently.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::{Arg, Command, CommandFactory};
use systemprompt_cli::args::Cli;

const SENTINEL: &str = "sentinel-do-not-leak";

fn is_sensitive(arg: &Arg) -> bool {
    let env = arg
        .get_env()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let long = arg.get_long().unwrap_or_default();
    systemprompt_logging::sanitize::is_redacted(&env)
        || systemprompt_logging::sanitize::is_redacted(long)
        || env.ends_with("_URL")
        || long.ends_with("-url")
        || long.ends_with("email")
}

fn walk(cmd: &Command, path: &str, leaking: &mut Vec<String>) {
    for arg in cmd.get_arguments() {
        if arg.get_env().is_none() || !is_sensitive(arg) {
            continue;
        }
        if !arg.is_hide_env_values_set() && !arg.is_hide_env_set() && !arg.is_hide_set() {
            leaking.push(format!("{path} {}", arg.get_id()));
        }
    }
    for sub in cmd.get_subcommands() {
        walk(sub, &format!("{path} {}", sub.get_name()), leaking);
    }
}

#[test]
fn no_sensitive_env_bound_arg_renders_its_value_in_help() {
    let mut leaking = Vec::new();
    walk(&Cli::command(), "systemprompt", &mut leaking);
    assert!(
        leaking.is_empty(),
        "these env-bound args would print their live value in --help; \
         add `hide_env_values = true`: {leaking:#?}"
    );
}

#[test]
fn admin_setup_help_names_the_provider_vars_without_their_values() {
    unsafe {
        std::env::set_var("GEMINI_API_KEY", SENTINEL);
        std::env::set_var("SYSTEMPROMPT_DB_PASSWORD", SENTINEL);
    }

    let mut cmd = Cli::command();
    let setup = cmd
        .find_subcommand_mut("admin")
        .expect("admin")
        .find_subcommand_mut("setup")
        .expect("setup");
    let help = setup.render_long_help().to_string();

    assert!(
        !help.contains(SENTINEL),
        "admin setup --help leaked an env value:\n{help}"
    );
    assert!(help.contains("GEMINI_API_KEY"), "{help}");
    assert!(help.contains("SYSTEMPROMPT_DB_PASSWORD"), "{help}");
}

#[test]
fn the_global_database_url_does_not_leak_into_an_unrelated_subcommand() {
    unsafe {
        std::env::set_var("SYSTEMPROMPT_DATABASE_URL", SENTINEL);
    }

    let mut cmd = Cli::command();
    let sub = cmd.find_subcommand_mut("admin").expect("admin");
    let help = sub.render_long_help().to_string();

    assert!(
        !help.contains(SENTINEL),
        "the global --database-url leaked into `admin --help`:\n{help}"
    );
}
