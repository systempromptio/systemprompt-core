use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingConfig {
    pub name: String,
    pub title: String,
    pub description: String,
    pub copyright: String,
    #[serde(rename = "themeColor")]
    pub theme_color: String,
    pub display_sitename: bool,
    pub twitter_handle: String,
    pub logo: LogoConfig,
    pub favicon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoConfig {
    pub primary: LogoVariant,
    #[serde(default)]
    pub dark: Option<LogoVariant>,
    #[serde(default)]
    pub small: Option<LogoVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoVariant {
    #[serde(default)]
    pub svg: Option<String>,
    #[serde(default)]
    pub webp: Option<String>,
    #[serde(default)]
    pub png: Option<String>,
}
