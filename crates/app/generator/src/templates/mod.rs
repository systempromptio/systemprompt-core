pub mod data;
mod engine;
pub mod html;
pub mod items;
pub mod navigation;

pub use data::{prepare_template_data, TemplateDataParams};
pub use engine::{get_assets_path, get_templates_path, load_web_config};
pub use navigation::{generate_footer_html, generate_social_action_bar_html};
