//! Template-engine glue: locates the on-disk template directory and loads
//! `web.yaml` configuration shared by every part of the generator pipeline.

mod engine;

pub use engine::{get_templates_path, load_web_config};
