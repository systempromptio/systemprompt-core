//! Failure reporting for the stdio display sinks.
//!
//! A display write is best-effort: the command's outcome does not depend on
//! whether the terminal accepted the bytes, so a failed write is reported
//! through `tracing` and the caller continues. A closed downstream pipe is
//! not reported at all — with SIGPIPE ignored every later write would fail
//! the same way and flood the log for a reader that has already gone away.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::ErrorKind;

pub(super) fn report_write_failure(sink: &'static str, error: &std::io::Error) {
    if error.kind() == ErrorKind::BrokenPipe {
        return;
    }
    tracing::warn!(sink, error = %error, "Display sink write failed");
}

pub(super) fn stdout_write(args: std::fmt::Arguments<'_>) {
    use std::io::Write;
    if let Err(error) = write!(std::io::stdout(), "{args}") {
        report_write_failure("stdout", &error);
    }
}

pub(super) fn stdout_writeln(args: std::fmt::Arguments<'_>) {
    use std::io::Write;
    if let Err(error) = writeln!(std::io::stdout(), "{args}") {
        report_write_failure("stdout", &error);
    }
}

pub(super) fn stderr_writeln(args: std::fmt::Arguments<'_>) {
    use std::io::Write;
    if let Err(error) = writeln!(std::io::stderr(), "{args}") {
        report_write_failure("stderr", &error);
    }
}
