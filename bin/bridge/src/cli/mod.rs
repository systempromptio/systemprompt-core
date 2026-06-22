pub mod args;
pub mod clean;
pub mod credential_helper;
pub mod diagnostics;
pub mod doctor;
mod gui;
mod install;
mod install_claude_policy;
pub mod login;
pub mod logout;
pub mod oauth_client;
pub mod output;
mod proxy;
mod run;
pub mod status;
pub mod sync;
mod uninstall;
pub mod validate;
pub mod whoami;

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
        Some("proxy") => proxy::cmd_proxy(),
        Some("login") => login::cmd_login(&args),
        Some("logout") => logout::cmd_logout(),
        Some("clean") => clean::cmd_clean(),
        Some("status") => status::cmd_status(),
        Some("whoami") => whoami::cmd_whoami(),
        Some("install") => install::cmd_install(&args),
        Some("__install-claude-policy") => install_claude_policy::cmd(&args),
        Some("sync") => sync::cmd_sync(&args),
        Some("oauth-client") => oauth_client::cmd_oauth_client(&args),
        Some("validate") => validate::cmd_validate(),
        Some("uninstall") => uninstall::cmd_uninstall(&args),
        Some("credential-helper") => credential_helper::cmd_credential_helper(&args),
        Some("diagnostics") => diagnostics::cmd_diagnostics(),
        Some("doctor") => doctor::cmd_doctor(),
        Some("gui") => gui::cmd_gui(),
        Some("--version" | "-V" | "version") => {
            output::print_str(&format!(
                "{} {} ({}, {})\n",
                crate::brand::brand().binary_name,
                env!("CARGO_PKG_VERSION"),
                diagnostics::short_sha(),
                diagnostics::GIT_COMMIT_DATE,
            ));
            ExitCode::SUCCESS
        },
        Some("help" | "--help" | "-h") => {
            output::print_str(&crate::help());
            ExitCode::SUCCESS
        },
        Some(other) => {
            diag(&format!("unknown command: {other}"));
            output::eprint_str(&crate::help());
            ExitCode::from(64)
        },
    }
}
