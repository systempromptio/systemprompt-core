//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::auth::setup;
use crate::cli::args::parse_opt_flag;
use crate::cli::output;
use crate::obs::output::diag;

pub fn cmd_login(args: &[String]) -> ExitCode {
    let token = match args.get(2) {
        Some(t) if !t.is_empty() => t.clone(),
        _ => {
            diag(&format!(
                "usage: {} login <sp-live-...> [--gateway <url>]",
                crate::brand::brand().binary_name
            ));
            return ExitCode::from(64);
        },
    };
    let gateway = parse_opt_flag(args, "--gateway");

    match setup::login(&token, gateway.as_deref()) {
        Ok(paths) => {
            let bin = crate::brand::brand().binary_name;
            output::print_line(&format!("Stored PAT for {bin} helper."));
            output::print_line(&format!("  config: {}", paths.config_file.display()));
            output::print_line(&format!("  secret: {} (0600)", paths.pat_file.display()));
            output::print_line(&format!("Next: run `{bin}` to fetch a JWT."));
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("login failed: {e}"));
            ExitCode::from(1)
        },
    }
}
