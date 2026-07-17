//! Template-engine glue: locates the on-disk template directory and loads
//! `web.yaml` configuration shared by every part of the generator pipeline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod engine;

pub use engine::{get_templates_path, load_web_config};
