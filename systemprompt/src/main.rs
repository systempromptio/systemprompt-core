//! The `systemprompt` binary: the CLI with every capability in `reference`.
//!
//! The capability crates are linked through this crate's library, which
//! re-exports each of them, so their link-time registrations are present.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

fn main() -> Result<ExitCode, systemprompt::RunError> {
    systemprompt::reference().run()
}
