//! Web rendering configuration: paths, branding, theme, layout, and
//! navigation. These types are serialized to / from YAML in the
//! `webconfig.yaml` of each profile.

mod branding;
mod error;
mod paths;
mod theme;

pub use branding::{BrandingConfig, LogoConfig, LogoVariant};
pub use error::WebConfigError;
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

/// Top-level web-rendering configuration loaded per profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebConfig {
    /// Filesystem paths driving template/asset discovery.
    pub paths: PathsConfig,
    /// Inline / external `<script>` tags injected into rendered pages.
    #[serde(default)]
    pub scripts: Vec<ScriptConfig>,
    /// Optional content-source registration block.
    #[serde(default)]
    pub content: Option<ContentConfig>,
    /// Brand identity (name, logo, copyright, ...).
    pub branding: BrandingConfig,
    /// Font-family declarations.
    pub fonts: FontsConfig,
    /// Light / dark colour palettes.
    pub colors: ColorsConfig,
    /// Typography scale.
    pub typography: TypographyConfig,
    /// Spacing scale.
    pub spacing: SpacingConfig,
    /// Border-radius scale.
    pub radius: RadiusConfig,
    /// Shadow tokens.
    pub shadows: ShadowsConfig,
    /// Animation timing tokens.
    pub animation: AnimationConfig,
    /// `z-index` token stack.
    #[serde(rename = "zIndex")]
    pub z_index: ZIndexConfig,
    /// Layout dimensions (header height, sidebar widths, ...).
    pub layout: LayoutConfig,
    /// Card-component design tokens.
    pub card: CardConfig,
    /// Mobile-breakpoint design tokens.
    pub mobile: MobileConfig,
    /// Touch-target sizing tokens.
    #[serde(rename = "touchTargets")]
    pub touch_targets: TouchTargetsConfig,
    /// Top-level navigation links.
    #[serde(default)]
    pub nav: NavConfig,
    /// Social-action-bar configuration.
    #[serde(default)]
    pub social_action_bar: SocialActionBarConfig,
    /// Free-form per-page configuration blobs keyed by page id.
    #[serde(default)]
    pub pages: HashMap<String, serde_json::Value>,
}

/// Top-level navigation links surfaced by the rendering host.
///
/// Field names drop the `_url` suffix on the Rust side (clippy
/// `struct_field_names`) but `#[serde(rename = "<name>_url")]` keeps the
/// YAML / JSON schema stable.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NavConfig {
    /// URL of the application / dashboard entry point.
    #[serde(default, rename = "app_url")]
    pub app: String,
    /// URL of the public documentation root.
    #[serde(default, rename = "docs_url")]
    pub docs: String,
    /// URL of the public blog root.
    #[serde(default, rename = "blog_url")]
    pub blog: String,
    /// URL of the playbooks root.
    #[serde(default, rename = "playbooks_url")]
    pub playbooks: String,
    /// URL of the GitHub organization or repository.
    #[serde(default, rename = "github_url")]
    pub github: String,
    /// URL of the getting-started landing page.
    #[serde(default, rename = "getting_started_url")]
    pub getting_started: String,
}

/// Configuration for the social-action bar rendered on content pages.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SocialActionBarConfig {
    /// Optional label rendered alongside the action buttons.
    #[serde(default)]
    pub label: String,
    /// Configured social platforms shown as buttons.
    #[serde(default)]
    pub platforms: Vec<SocialPlatform>,
    /// Whether to render a copy-link / share button.
    #[serde(default)]
    pub enable_share: bool,
}

/// One social-platform entry in [`SocialActionBarConfig::platforms`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SocialPlatform {
    /// Platform identifier (`twitter`, `mastodon`, ...).
    #[serde(rename = "type")]
    pub platform_type: String,
    /// Optional URL the button links to.
    #[serde(default)]
    pub url: Option<String>,
    /// Optional accessible label / tooltip.
    #[serde(default)]
    pub label: Option<String>,
}
