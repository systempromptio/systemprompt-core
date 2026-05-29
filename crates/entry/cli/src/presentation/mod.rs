//! Live terminal rendering of the service-startup sequence.
//!
//! Consumes the startup event stream and drives spinners, a service table, and
//! completion messaging. [`StartupRenderer`] is the entry point; rendering
//! state and individual widgets are internal.

mod renderer;
mod state;
mod widgets;

pub use renderer::StartupRenderer;
