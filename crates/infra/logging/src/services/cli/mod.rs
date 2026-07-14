//! Terminal-display layer for CLI output.
//!
//! Renders human-facing status, tables, and progress to stderr/stdout through
//! [`CliService`] and the [`Display`] trait. This is one of the sanctioned
//! `println!`/`write!`-to-stdio sinks; it deliberately does not route through
//! `tracing`. Themed colour and iconography live in [`theme`].

mod banners;
pub mod display;
mod macros;
mod service;
pub mod startup;
pub mod table;
pub mod theme;
mod types;

pub use display::{Display, DisplayUtils};
pub use service::CliService;
pub use startup::{
    render_phase_header, render_phase_info, render_phase_success, render_phase_warning,
    render_startup_banner,
};
pub use table::{ServiceTableEntry, render_service_table, render_startup_complete, render_table};
pub use theme::{
    ActionType, BrandColors, ColorType, Colors, EmphasisType, IconType, Icons, ItemStatus,
    MessageLevel, ModuleType, ServiceStatus, Theme,
};

use super::output;
