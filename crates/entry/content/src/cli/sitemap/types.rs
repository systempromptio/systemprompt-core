//! Serializable output shapes for the `web sitemap` commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SitemapRoute {
    pub url: String,
    pub priority: f32,
    pub changefreq: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SitemapGenerateOutput {
    pub output_path: String,
    pub routes_count: usize,
    pub message: String,
}
