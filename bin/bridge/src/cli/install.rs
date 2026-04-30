use std::process::ExitCode;

use systemprompt_identifiers::ValidatedUrl;

use crate::cli::args::{has_flag, parse_opt_flag};
use crate::cli::output;
use crate::ids::PinnedPubKey;
use crate::install;
use crate::obs::output::diag;
use crate::schedule::Os;

pub(crate) fn cmd_install(args: &[String]) -> ExitCode {
    let print_mdm = parse_opt_flag(args, "--print-mdm")
        .as_deref()
        .and_then(Os::parse);
    let emit_sched = parse_opt_flag(args, "--emit-schedule-template")
        .as_deref()
        .and_then(Os::parse);
    let gateway = match parse_opt_flag(args, "--gateway") {
        Some(raw) => match ValidatedUrl::try_new(raw.trim()) {
            Ok(url) => Some(url),
            Err(e) => {
                diag(&format!("--gateway: invalid URL: {e}"));
                return ExitCode::from(64);
            },
        },
        None => None,
    };
    let pubkey = parse_opt_flag(args, "--pubkey").map(PinnedPubKey::new);
    let apply = has_flag(args, "--apply");
    let apply_mobileconfig = has_flag(args, "--apply-mobileconfig");
    match install::install(&install::InstallOptions {
        print_mdm,
        emit_schedule_template: emit_sched,
        gateway_url: gateway,
        pubkey,
        apply,
        apply_mobileconfig,
    }) {
        Ok(summary) => {
            output::print_str(&install::render_install_summary(&summary));
            ExitCode::SUCCESS
        },
        Err(err) => {
            diag(&err.to_string());
            install::InstallError::EXIT_CODE
        },
    }
}
