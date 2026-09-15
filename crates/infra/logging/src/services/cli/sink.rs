//! The stdio display sinks; failures are reported through the infra-wide
//! [`report_write_failure`] policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) use systemprompt_database::services::display::report_write_failure;

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
