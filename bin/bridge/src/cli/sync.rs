use std::process::ExitCode;
use std::time::Duration;

use crate::cli::args::{has_flag, parse_opt_flag};
use crate::cli::output;
use crate::sync;

pub(crate) fn cmd_sync(args: &[String]) -> ExitCode {
    let watch = has_flag(args, "--watch");
    let interval = parse_opt_flag(args, "--interval").and_then(|s| s.parse().ok());
    let allow_unsigned = has_flag(args, "--allow-unsigned");
    let force_replay = has_flag(args, "--force-replay");
    let allow_tofu = has_flag(args, "--allow-tofu");

    sync::warn_unsafe_flags(allow_unsigned, force_replay, allow_tofu);

    if !watch {
        return run_once_print(allow_unsigned, force_replay, allow_tofu);
    }

    let secs = interval.unwrap_or(1800).max(sync::WATCH_FLOOR_SECS);
    loop {
        let code = run_once_print(allow_unsigned, force_replay, allow_tofu);
        if code != ExitCode::SUCCESS {
            tracing::warn!(retry_in_secs = secs, "sync: non-zero exit; retrying");
        }
        std::thread::sleep(Duration::from_secs(secs));
    }
}

fn run_once_print(allow_unsigned: bool, force_replay: bool, allow_tofu: bool) -> ExitCode {
    let result = match crate::proxy::block_on(sync::run_once(
        allow_unsigned,
        force_replay,
        allow_tofu,
    )) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("runtime init failed: {e}");
            return ExitCode::from(70);
        },
    };
    match result {
        Ok(summary) => {
            output::print_line(&summary.one_line());
            ExitCode::SUCCESS
        },
        Err(err) => {
            let exit = err.exit_code();
            tracing::error!("{err}");
            exit
        },
    }
}
