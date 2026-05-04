//! Typography sizes and weights.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographyConfig {
    pub sizes: TypographySizes,
    pub weights: TypographyWeights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographySizes {
    pub xs: String,
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub xl: String,
    pub xxl: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TypographyWeights {
    pub regular: u16,
    pub medium: u16,
    pub semibold: u16,
    pub bold: u16,
}
