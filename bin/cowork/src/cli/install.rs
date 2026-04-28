use std::process::ExitCode;

use crate::cli::args::{has_flag, parse_opt_flag};
use crate::install;
use crate::schedule::Os;

pub(crate) fn cmd_install(args: &[String]) -> ExitCode {
    let print_mdm = parse_opt_flag(args, "--print-mdm")
        .as_deref()
        .and_then(Os::parse);
    let emit_sched = parse_opt_flag(args, "--emit-schedule-template")
        .as_deref()
        .and_then(Os::parse);
    let gateway = parse_opt_flag(args, "--gateway");
    let pubkey = parse_opt_flag(args, "--pubkey");
    let apply = has_flag(args, "--apply");
    let apply_mobileconfig = has_flag(args, "--apply-mobileconfig");
    install::install(install::InstallOptions {
        print_mdm,
        emit_schedule_template: emit_sched,
        gateway_url: gateway,
        pubkey,
        apply,
        apply_mobileconfig,
    })
}
