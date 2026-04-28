use std::process::ExitCode;

use crate::cli::args::{has_flag, parse_opt_flag};
use crate::sync;

pub(crate) fn cmd_sync(args: &[String]) -> ExitCode {
    let watch = has_flag(args, "--watch");
    let interval = parse_opt_flag(args, "--interval").and_then(|s| s.parse().ok());
    let allow_unsigned = has_flag(args, "--allow-unsigned");
    let force_replay = has_flag(args, "--force-replay");
    let allow_tofu = has_flag(args, "--allow-tofu");
    sync::sync(sync::SyncOptions {
        watch,
        interval,
        allow_unsigned,
        force_replay,
        allow_tofu,
    })
}
