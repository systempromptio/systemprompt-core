//! Edge-triggered desktop notifications.
//!
//! The GUI recomputes gateway, MCP-auth and session state every probe interval.
//! Notifying from those sites directly would raise the same toast every thirty
//! seconds for as long as the condition held, so every signal here fires once
//! on the transition *into* a state and stays quiet until it clears.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{GuiApp, window};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signal {
    GatewayUnreachable,
    McpAuthBroken,
    SessionExpiring,
}

impl GuiApp {
    pub(crate) fn signal_raised(&mut self, signal: Signal, title: &str, message: &str) {
        if !self.active_signals.insert(signal) {
            return;
        }
        window::notify_user(title, message);
    }

    pub(crate) fn signal_cleared(&mut self, signal: Signal) {
        _ = self.active_signals.remove(&signal);
    }
}
