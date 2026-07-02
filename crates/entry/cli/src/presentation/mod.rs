//! Terminal presentation widgets.
//!
//! Live rendering of the service-startup sequence ([`StartupRenderer`] is the
//! entry point; its state and widgets are internal) plus the shared [`tables`]
//! widgets that shape command records into rendered `tabled` output.

mod renderer;
mod state;
pub mod tables;
mod widgets;

pub use renderer::StartupRenderer;
