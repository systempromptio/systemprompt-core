//! Assembling the runtime config and the forwarded argument vector.
//!
//! Split from the clap definitions next door because these are the only pieces
//! with behaviour rather than shape: the verbosity/output precedence, and the
//! reconstruction that decides what a profile-routed subprocess actually
//! receives.
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

pub fn reconstruct_args(cli: &Cli) -> Vec<String> {
    let original: Vec<String> = std::env::args().skip(1).collect();
    reconstruct_args_from(cli, &original)
}

// Why: takes the original argv rather than reading `std::env::args()` so the
// reconstruction can be exercised at all. Reading the process inside the
// function made every branch below reachable only by however the test binary
// itself happened to be invoked.
pub fn reconstruct_args_from(cli: &Cli, original_args: &[String]) -> Vec<String> {
    let mut args = Vec::new();

    if cli.verbosity.debug {
        args.push("--debug".to_owned());
    } else if cli.verbosity.verbose {
        args.push("--verbose".to_owned());
    } else if cli.verbosity.quiet {
        args.push("--quiet".to_owned());
    }

    if cli.output.json {
        args.push("--json".to_owned());
    } else if cli.output.yaml {
        args.push("--yaml".to_owned());
    }

    if cli.display.no_color {
        args.push("--no-color".to_owned());
    }

    if cli.display.non_interactive {
        args.push("--non-interactive".to_owned());
    }

    if let Some(ref profile) = cli.profile_opts.profile {
        args.push("--profile".to_owned());
        args.push(profile.clone());
    }

    let mut skip_next = false;
    for arg in original_args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--profile" {
            skip_next = true;
            continue;
        }
        if arg.starts_with("--profile=") {
            continue;
        }
        if !args.contains(arg)
            && !matches!(
                arg.as_str(),
                "--debug"
                    | "--verbose"
                    | "-v"
                    | "--quiet"
                    | "-q"
                    | "--json"
                    | "--yaml"
                    | "--no-color"
                    | "--non-interactive"
            )
        {
            args.push(arg.clone());
        }
    }

    args
}

pub fn has_local_export_flag(command: Option<&Commands>) -> bool {
    let args: Vec<String> = std::env::args().collect();
    has_local_export_flag_in(command, &args)
}

// Why: same split as `reconstruct_args_from`. The `--export` check is one line
// of logic wrapped around a process read, and the read is what made it
// untestable.
pub fn has_local_export_flag_in(command: Option<&Commands>, args: &[String]) -> bool {
    if !matches!(command, Some(Commands::Analytics(_))) {
        return false;
    }
    args.iter()
        .any(|arg| arg == "--export" || arg.starts_with("--export="))
}
