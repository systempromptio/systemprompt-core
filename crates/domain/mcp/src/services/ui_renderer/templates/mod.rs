//! Concrete [`UiRenderer`](super::UiRenderer) implementations, one per
//! artifact type (chart, dashboard, form, image, list, table, text).
//! [`html`] provides the shared HTML-escaping and element helpers.

mod chart;
mod dashboard;
mod form;
mod form_field;
pub mod html;
mod image;
mod list;
mod table;
mod text;

pub use chart::ChartRenderer;
pub use dashboard::DashboardRenderer;
pub use form::FormRenderer;
pub use image::ImageRenderer;
pub use list::ListRenderer;
pub use table::TableRenderer;
pub use text::TextRenderer;
