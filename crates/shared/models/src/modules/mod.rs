//! Path constants and service-category classification shared across API/CLI
//! surfaces.

mod api_paths;
mod cli_paths;
mod service_category;

pub use api_paths::ApiPaths;
pub use cli_paths::CliPaths;
pub use service_category::ServiceCategory;
