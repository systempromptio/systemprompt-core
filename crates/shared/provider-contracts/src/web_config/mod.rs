mod branding;
mod error;
mod navigation;
mod paths;
mod theme;

pub use branding::{BrandingConfig, LogoConfig, LogoVariant};
pub use error::WebConfigError;
pub use navigation::{
    FooterConfig, NavConfig, NavLink, NavigationConfig, SocialActionBar, SocialLink, SocialPlatform,
};
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
    pub navigation: NavigationConfig,
    pub social_action_bar: SocialActionBar,
    #[serde(default)]
    pub pages: HashMap<String, serde_json::Value>,
    pub nav: NavConfig,
}
