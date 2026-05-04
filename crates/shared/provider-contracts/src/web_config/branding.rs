//! Brand-identity configuration: name, logo, social handles, copyright.

use serde::{Deserialize, Serialize};

/// Top-level brand identity surfaced in templates and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingConfig {
    /// Brand display name (e.g. `systemprompt.io`).
    pub name: String,
    /// HTML `<title>` and OG default.
    pub title: String,
    /// Brand description used as the meta description fallback.
    pub description: String,
    /// Footer copyright string.
    pub copyright: String,
    /// PWA `theme-color` value.
    #[serde(rename = "themeColor")]
    pub theme_color: String,
    /// Whether to render the brand name next to the logo.
    pub display_sitename: bool,
    /// Twitter / X handle without the leading `@`.
    pub twitter_handle: String,
    /// Logo asset variants.
    pub logo: LogoConfig,
    /// Path to the favicon asset.
    pub favicon: String,
}

/// Logo asset variants (light / dark / small).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoConfig {
    /// Default logo variant used in light mode.
    pub primary: LogoVariant,
    /// Optional dark-mode variant.
    #[serde(default)]
    pub dark: Option<LogoVariant>,
    /// Optional reduced-size variant for compact contexts.
    #[serde(default)]
    pub small: Option<LogoVariant>,
}

/// One logo asset rendered as SVG / WebP / PNG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoVariant {
    /// Path to the SVG file, when available.
    #[serde(default)]
    pub svg: Option<String>,
    /// Path to the WebP file, when available.
    #[serde(default)]
    pub webp: Option<String>,
    /// Path to the PNG file, when available.
    #[serde(default)]
    pub png: Option<String>,
}
