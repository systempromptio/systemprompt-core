//! GUI quit handler: ends the event loop so `gui::run` returns and the CLI
//! wrapper performs the shutdown sequence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use winit::event_loop::ActiveEventLoop;

pub(crate) fn on_quit(event_loop: &dyn ActiveEventLoop) {
    tracing::info!("gui quit requested; leaving the event loop");
    event_loop.exit();
}
