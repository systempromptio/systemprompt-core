//! Executable entry point for the bridge binary; delegates to
//! `systemprompt_bridge::run`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    systemprompt_bridge::run()
}
