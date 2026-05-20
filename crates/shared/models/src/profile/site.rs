use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SiteConfig {
    pub name: String,

    #[serde(default)]
    pub github_link: Option<String>,
}
