mod branding;
mod error;
mod paths;
mod theme;

pub use branding::{BrandingConfig, LogoConfig, LogoVariant};
pub use error::WebConfigError;
pub use paths::{ContentConfig, PathsConfig, ScriptConfig};
pub use theme::{
    AnimationConfig, CardConfig, CardGradient, CardPadding, CardRadius, ColorPalette, ColorsConfig,
    FontConfig, FontFile, FontsConfig, LayoutConfig, MobileConfig, MobileLayout, MobileTypography,
    PrimaryColor, RadiusConfig, ShadowSet, ShadowsConfig, SidebarConfig, SpacingConfig,
    TouchTargetsConfig, TypographyConfig, TypographySizes, TypographyWeights, ZIndexConfig,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebConfig {
    pub paths: PathsConfig,
    #[serde(default)]
    pub scripts: Vec<ScriptConfig>,
    #[serde(default)]
    pub content: Option<ContentConfig>,
    pub branding: BrandingConfig,
    pub fonts: FontsConfig,
    pub colors: ColorsConfig,
    pub typography: TypographyConfig,
    pub spacing: SpacingConfig,
    pub radius: RadiusConfig,
    pub shadows: ShadowsConfig,
    pub animation: AnimationConfig,
    #[serde(rename = "zIndex")]
    pub z_index: ZIndexConfig,
    pub layout: LayoutConfig,
    pub card: CardConfig,
    pub mobile: MobileConfig,
    #[serde(rename = "touchTargets")]
    pub touch_targets: TouchTargetsConfig,
    #[serde(default)]
    pub nav: NavConfig,
    #[serde(default)]
    pub social_action_bar: SocialActionBarConfig,
    #[serde(default)]
    pub pages: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct NavConfig {
    #[serde(default)]
    pub app_url: String,
    #[serde(default)]
    pub docs_url: String,
    #[serde(default)]
    pub blog_url: String,
    #[serde(default)]
    pub playbooks_url: String,
    #[serde(default)]
    pub github_url: String,
    #[serde(default)]
    pub getting_started_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SocialActionBarConfig {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub platforms: Vec<SocialPlatform>,
    #[serde(default)]
    pub enable_share: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SocialPlatform {
    #[serde(rename = "type")]
    pub platform_type: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}
