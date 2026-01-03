use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub name: String,

    #[serde(default)]
    pub github_link: Option<String>,
}
