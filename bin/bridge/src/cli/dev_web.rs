//! `dev-web` — serve the GUI web tree over HTTP for browser-based development.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::dev_preview::{Options, fixtures, serve};
use crate::obs::output::diag;

const DEFAULT_PORT: u16 = 4310;

pub(super) fn cmd_dev_web(args: &[String]) -> ExitCode {
    let mut port = DEFAULT_PORT;
    let mut web_root = fixtures::default_web_root();
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                let Some(v) = args.get(i + 1).and_then(|v| v.parse::<u16>().ok()) else {
                    diag("dev-web: --port needs a number");
                    return ExitCode::from(64);
                };
                port = v;
                i += 2;
            },
            "--web-root" => {
                let Some(v) = args.get(i + 1) else {
                    diag("dev-web: --web-root needs a path");
                    return ExitCode::from(64);
                };
                web_root = std::path::PathBuf::from(v);
                i += 2;
            },
            other => {
                diag(&format!("dev-web: unknown flag {other}"));
                return ExitCode::from(64);
            },
        }
    }
    if !web_root.is_dir() {
        diag(&format!(
            "dev-web: web root {} is not a directory",
            web_root.display()
        ));
        return ExitCode::from(66);
    }
    match serve(&Options { port, web_root }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            diag(&format!("dev-web: {e}"));
            ExitCode::from(1)
        },
    }
}
