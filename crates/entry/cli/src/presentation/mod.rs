//! Terminal presentation widgets.
//!
//! Live rendering of the service-startup sequence ([`StartupRenderer`] is the
//! entry point; its state and widgets are internal) plus the shared [`tables`]
//! widgets that shape command records into rendered `tabled` output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod renderer;
mod state;
pub mod tables;
mod widgets;

pub use renderer::StartupRenderer;
