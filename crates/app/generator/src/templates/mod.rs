pub mod data;
mod engine;

pub use data::{prepare_template_data, TemplateDataParams};
pub use engine::{get_assets_path, get_templates_path, load_web_config};
