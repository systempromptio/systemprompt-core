pub mod data;
mod engine;
pub mod html;
pub mod items;
pub mod navigation;
pub mod paper;

pub use data::{prepare_template_data, TemplateDataParams};
pub use engine::{get_assets_path, get_templates_path, load_web_config, TemplateEngine};
pub use navigation::{generate_footer_html, generate_social_action_bar_html};
pub use paper::{
    calculate_read_time, generate_toc_html, parse_paper_metadata, render_paper_sections_html,
};
