//! Site identity block of the profile (title, URLs, branding).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SiteConfig {
    pub name: String,

    #[serde(default)]
    pub github_link: Option<String>,
}
