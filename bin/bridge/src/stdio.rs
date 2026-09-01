//! The bridge's user-facing stdout/stderr sink — the one place a CLI line is
//! printed, so `print_stdout` stays denied everywhere else.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "single user-facing display sink for the bridge CLI; analogue of \
              crates/infra/logging/services/cli"
)]

pub fn print_line(msg: &str) {
    println!("{msg}");
}

pub fn print_str(msg: &str) {
    print!("{msg}");
}

pub fn eprint_str(msg: &str) {
    eprint!("{msg}");
}

pub fn emit_json<T: serde::Serialize>(value: &T) -> std::io::Result<()> {
    use std::io::Write;
    let json = serde_json::to_string(value)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(json.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}

// Why: routed through tracing so it lands in the log file as well as on the
// console the subscriber tees to.
pub fn diag(msg: &str) {
    tracing::warn!(target: "systemprompt_bridge", "{msg}");
}
