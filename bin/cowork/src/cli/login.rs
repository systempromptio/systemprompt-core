use std::process::ExitCode;

use crate::auth::setup;
use crate::cli::args::parse_opt_flag;
use crate::obs::output::diag;

pub(crate) fn cmd_login(args: &[String]) -> ExitCode {
    let token = match args.get(2) {
        Some(t) if !t.is_empty() => t.clone(),
        _ => {
            diag("usage: systemprompt-cowork login <sp-live-...> [--gateway <url>]");
            return ExitCode::from(64);
        },
    };
    let gateway = parse_opt_flag(args, "--gateway");

    match setup::login(&token, gateway.as_deref()) {
        Ok(paths) => {
            println!("Stored PAT for systemprompt-cowork helper.");
            println!("  config: {}", paths.config_file.display());
            println!("  secret: {} (0600)", paths.pat_file.display());
            println!("Next: run `systemprompt-cowork` to fetch a JWT.");
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("login failed: {e}"));
            ExitCode::from(1)
        },
    }
}
