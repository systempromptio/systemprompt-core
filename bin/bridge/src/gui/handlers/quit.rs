//! GUI quit handler: ends the event loop so `gui::run` returns and the CLI
//! wrapper performs the shutdown sequence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use winit::event_loop::ActiveEventLoop;

use crate::gui::GuiApp;

pub(crate) fn on_quit(app: &mut GuiApp, event_loop: &dyn ActiveEventLoop) {
    tracing::info!("gui quit requested; leaving the event loop");
    app.stop_server();
    event_loop.exit();
}
