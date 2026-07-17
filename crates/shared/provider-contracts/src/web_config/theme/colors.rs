//! Light / dark colour palette types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorsConfig {
    pub light: ColorPalette,
    pub dark: ColorPalette,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPalette {
    pub primary: PrimaryColor,
    pub secondary: PrimaryColor,
    pub success: String,
    pub warning: String,
    pub error: String,
    pub surface: SurfaceColors,
    pub text: TextColors,
    pub background: BackgroundColors,
    pub border: BorderColors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryColor {
    pub hsl: String,
    pub rgb: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceColors {
    pub default: String,
    pub dark: String,
    pub variant: String,
    #[serde(rename = "secondaryContainer")]
    pub secondary_container: String,
    #[serde(rename = "errorContainer")]
    pub error_container: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextColors {
    pub primary: String,
    pub secondary: String,
    pub inverted: String,
    pub disabled: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundColors {
    pub default: String,
    pub dark: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderColors {
    pub default: String,
    pub dark: String,
    pub outline: String,
}
