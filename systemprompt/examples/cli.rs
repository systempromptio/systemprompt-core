//! Boots the standard `systemprompt` CLI through the facade: the reference
//! composition, run as a command-line binary.
//!
//! Run with: `cargo run -p systemprompt --example cli --features full --
//! --help`

use std::process::ExitCode;

use systemprompt::{RunError, reference};

fn main() -> Result<ExitCode, RunError> {
    reference().run()
}
