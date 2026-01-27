use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationConfig {
    pub footer: FooterConfig,
    pub social: Vec<SocialLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FooterConfig {
    pub legal: Vec<NavLink>,
    #[serde(default)]
    pub resources: Vec<NavLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavLink {
    pub path: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLink {
    pub href: String,
    #[serde(rename = "type")]
    pub link_type: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialActionBar {
    pub label: String,
    pub platforms: Vec<SocialPlatform>,
    pub enable_share: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialPlatform {
    #[serde(rename = "type")]
    pub platform_type: String,
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavConfig {
    pub app_url: String,
    pub docs_url: String,
    pub blog_url: String,
}
