//! Assembling the runtime config and the forwarded argument vector.
//!
//! Split from the clap definitions next door because these are the only pieces
//! with behaviour rather than shape: the verbosity/output precedence, and the
//! reconstruction that decides what a profile-routed subprocess actually
//! receives.
//!
//! The forwarded argv is the operator's argv edited by position: `--profile`
//! and `--database-url` (both spellings) name local targets and are dropped,
//! global flags typed before the subcommand move to just after the top-level
//! group (the remote gateway requires a subcommand first), and every other
//! token is kept verbatim and in order — a repeated value is never collapsed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Cli, Commands};
use crate::cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
use crate::env_overrides::EnvOverrides;

pub fn build_cli_config(cli: &Cli, env: &EnvOverrides) -> CliConfig {
    let mut cfg = CliConfig::resolve(env);

    if cli.verbosity.debug {
        cfg = cfg.with_verbosity(VerbosityLevel::Debug);
    } else if cli.verbosity.verbose {
        cfg = cfg.with_verbosity(VerbosityLevel::Verbose);
    } else if cli.verbosity.quiet {
        cfg = cfg.with_verbosity(VerbosityLevel::Quiet);
    }

    if cli.output.json {
        cfg = cfg.with_output_format(OutputFormat::Json);
    } else if cli.output.yaml {
        cfg = cfg.with_output_format(OutputFormat::Yaml);
    }

    if cli.display.no_color {
        cfg = cfg.with_color_mode(ColorMode::Never);
    }

    if cli.display.non_interactive {
        cfg = cfg.with_interactive(false);
    }

    cfg = cfg.with_profile_override(cli.profile_opts.profile.clone());

    cfg
}

pub fn reconstruct_args() -> Vec<String> {
    let original: Vec<String> = std::env::args().skip(1).collect();
    reconstruct_args_from(&original)
}

pub fn reconstruct_args_from(original_args: &[String]) -> Vec<String> {
    let mut leading_globals = Vec::new();
    let mut tokens = original_args.iter();
    let mut subcommand = None;

    while let Some(arg) = tokens.next() {
        if is_local_only_flag(arg) {
            if takes_separate_value(arg) {
                tokens.next();
            }
            continue;
        }
        if arg.starts_with('-') {
            leading_globals.push(arg.clone());
            continue;
        }
        subcommand = Some(arg.clone());
        break;
    }

    let Some(subcommand) = subcommand else {
        return leading_globals;
    };

    let mut forwarded = vec![subcommand];
    forwarded.extend(leading_globals);

    let mut after_terminator = false;
    while let Some(arg) = tokens.next() {
        if after_terminator {
            forwarded.push(arg.clone());
            continue;
        }
        if arg == "--" {
            after_terminator = true;
            forwarded.push(arg.clone());
            continue;
        }
        if is_local_only_flag(arg) {
            if takes_separate_value(arg) {
                tokens.next();
            }
            continue;
        }
        forwarded.push(arg.clone());
    }

    forwarded
}

const LOCAL_ONLY_FLAGS: [&str; 2] = ["--profile", "--database-url"];

fn takes_separate_value(arg: &str) -> bool {
    LOCAL_ONLY_FLAGS.contains(&arg)
}

fn is_local_only_flag(arg: &str) -> bool {
    LOCAL_ONLY_FLAGS.iter().any(|flag| {
        arg == *flag
            || arg
                .strip_prefix(flag)
                .is_some_and(|rest| rest.starts_with('='))
    })
}

pub fn has_local_export_flag(command: Option<&Commands>) -> bool {
    let args: Vec<String> = std::env::args().collect();
    has_local_export_flag_in(command, &args)
}

pub fn has_local_export_flag_in(command: Option<&Commands>, args: &[String]) -> bool {
    if !matches!(command, Some(Commands::Analytics(_))) {
        return false;
    }
    args.iter()
        .any(|arg| arg == "--export" || arg.starts_with("--export="))
}
