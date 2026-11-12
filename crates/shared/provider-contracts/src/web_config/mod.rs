//! Web rendering configuration: paths, branding, theme, layout and
//! per-page data. These types are serialized to / from YAML in the
//! `webconfig.yaml` of each profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod branding;
mod error;
mod i18n;
mod paths;
mod theme;

pub use branding::{BrandingConfig, LogoConfig, LogoVariant};
pub use error::WebConfigError;
pub use i18n::SiteI18nConfig;
pub use paths::{ContentConfig, PathsConfig, ScriptConfig};
pub use theme::{
    AnimationConfig, BackgroundColors, BorderColors, CardConfig, CardGradient, CardPadding,
    CardRadius, ColorPalette, ColorsConfig, FontConfig, FontFile, FontsConfig, LayoutConfig,
    MobileCardConfig, MobileConfig, MobileLayout, MobileTypography, PrimaryColor, RadiusConfig,
    ShadowSet, ShadowsConfig, SidebarConfig, SpacingConfig, SurfaceColors, TextColors,
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
    // JSON: per-page template data is free-form YAML owned by the site's
    // templates; the engine passes it through to Tera untouched.
    #[serde(default)]
    // JSON: Per-page template variables from the profile YAML; the page data model is dynamic.
    pub pages: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub i18n: SiteI18nConfig,
}
