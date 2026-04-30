pub(crate) mod args;
mod clean;
mod credential_helper;
pub mod diagnostics;
mod gui;
mod install;
mod login;
mod logout;
pub mod output;
mod run;
mod status;
mod sync;
mod uninstall;
mod validate;
mod whoami;

use std::env;
use std::process::ExitCode;

use crate::obs::output::diag;

pub fn run() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 && args::should_default_to_gui() {
        return gui::cmd_gui();
    }
    match args.get(1).map(String::as_str) {
        None | Some("run") => run::cmd_run(),
        Some("login") => login::cmd_login(&args),
        Some("logout") => logout::cmd_logout(),
        Some("clean") => clean::cmd_clean(),
        Some("status") => status::cmd_status(),
        Some("whoami") => whoami::cmd_whoami(),
        Some("install") => install::cmd_install(&args),
        Some("sync") => sync::cmd_sync(&args),
        Some("validate") => validate::cmd_validate(),
        Some("uninstall") => uninstall::cmd_uninstall(&args),
        Some("credential-helper") => credential_helper::cmd_credential_helper(&args),
        Some("diagnostics") => diagnostics::cmd_diagnostics(),
        Some("gui") => gui::cmd_gui(),
        Some("--version" | "-V" | "version") => {
            output::print_str(&format!(
                "systemprompt-bridge {} ({}, {})\n",
                env!("CARGO_PKG_VERSION"),
                diagnostics::short_sha(),
                diagnostics::GIT_COMMIT_DATE,
            ));
            ExitCode::SUCCESS
        },
        Some("help" | "--help" | "-h") => {
            output::print_str(crate::help());
            ExitCode::SUCCESS
        },
        Some(other) => {
            diag(&format!("unknown command: {other}"));
            output::eprint_str(crate::help());
            ExitCode::from(64)
        },
    }
}
