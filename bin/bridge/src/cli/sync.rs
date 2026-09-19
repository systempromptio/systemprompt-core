//! `sync` command: one-shot manifest sync with signing/replay/TOFU flags.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;
use std::time::Duration;

use crate::cli::args::{has_flag, parse_opt_flag};
use crate::context::BridgeContext;
use crate::gateway::Freshness;
use crate::{stdio, sync};

pub fn cmd_sync(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    let watch = has_flag(args, "--watch");
    let interval = match parse_opt_flag(args, "--interval") {
        None => None,
        Some(raw) => match raw.parse::<u64>() {
            Ok(secs) => Some(secs),
            Err(e) => {
                stdio::eprint_line(&format!(
                    "--interval: {raw:?} is not a number of seconds ({e})"
                ));
                return ExitCode::from(64);
            },
        },
    };
    let options = sync::SyncOptions {
        allow_unsigned: has_flag(args, "--allow-unsigned"),
        force_replay: has_flag(args, "--force-replay"),
        allow_tofu: has_flag(args, "--allow-tofu"),
        freshness: if has_flag(args, "--fresh") {
            Freshness::Fresh
        } else {
            Freshness::Memo
        },
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    sync::warn_unsafe_flags(
        options.allow_unsigned,
        options.force_replay,
        options.allow_tofu,
    );

    if !watch {
        return run_once_print(ctx, &options);
    }

    let secs = interval.unwrap_or(1800).max(sync::WATCH_FLOOR_SECS);
    loop {
        let code = run_once_print(ctx, &options);
        if code != ExitCode::SUCCESS {
            tracing::warn!(retry_in_secs = secs, "sync: non-zero exit; retrying");
        }
        std::thread::sleep(Duration::from_secs(secs));
    }
}

fn run_once_print(ctx: &BridgeContext, options: &sync::SyncOptions) -> ExitCode {
    let result = ctx.block_on(sync::run_once(ctx, options));
    match result {
        Ok(summary) => {
            stdio::print_line(&summary.one_line());
            ExitCode::SUCCESS
        },
        Err(err) => {
            let exit = err.exit_code();
            tracing::error!(error = %err, "sync failed");
            exit
        },
    }
}
