//! Light / dark colour palette types.

use serde::{Deserialize, Serialize};

/// Light / dark colour palettes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorsConfig {
    /// Light-mode palette.
    pub light: ColorPalette,
    /// Dark-mode palette.
    pub dark: ColorPalette,
}

/// One full colour palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPalette {
    /// Primary brand colour.
    pub primary: PrimaryColor,
    /// Secondary brand colour.
    pub secondary: PrimaryColor,
    /// Success-state colour.
    pub success: String,
    /// Warning-state colour.
    pub warning: String,
    /// Error-state colour.
    pub error: String,
    /// Surface colours (cards, sheets).
    pub surface: SurfaceColors,
    /// Text colours.
    pub text: TextColors,
    /// Background colours.
    pub background: BackgroundColors,
    /// Border colours.
    pub border: BorderColors,
}

/// Brand colour described in HSL plus a precomputed RGB triple.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryColor {
    /// CSS `hsl(...)` representation.
    pub hsl: String,
    /// Equivalent RGB byte triple.
    pub rgb: [u8; 3],
}

/// Surface colours used for cards, sheets, and containers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceColors {
    /// Default surface colour.
    pub default: String,
    /// Dark-emphasis surface colour.
    pub dark: String,
    /// Variant surface colour.
    pub variant: String,
    /// Container colour for the secondary brand colour.
    #[serde(rename = "secondaryContainer")]
    pub secondary_container: String,
    /// Container colour for the error state.
    #[serde(rename = "errorContainer")]
    pub error_container: String,
}

/// Text colour tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextColors {
    /// Primary text colour.
    pub primary: String,
    /// Secondary / muted text colour.
    pub secondary: String,
    /// Inverted text colour for use on dark surfaces.
    pub inverted: String,
    /// Disabled-state text colour.
    pub disabled: String,
}

/// Background colour tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundColors {
    /// Default page background.
    pub default: String,
    /// Dark-mode background.
    pub dark: String,
}

/// Border colour tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderColors {
    /// Default border colour.
    pub default: String,
    /// Dark-mode border colour.
    pub dark: String,
    /// Outline / focus-ring colour.
    pub outline: String,
}
