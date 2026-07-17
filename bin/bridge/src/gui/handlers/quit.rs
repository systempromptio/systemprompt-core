//! GUI quit handler.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[expect(
    clippy::exit,
    reason = "GUI quit handler intentionally terminates the process"
)]
pub(crate) fn on_quit() {
    std::process::exit(0);
}
