//! Typography sizes and weights.

use serde::{Deserialize, Serialize};

/// Typography configuration: sizes + weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographyConfig {
    /// Font sizes scale.
    pub sizes: TypographySizes,
    /// Font weights scale.
    pub weights: TypographyWeights,
}

/// Font-size scale (xs..xxl).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographySizes {
    /// Extra-small.
    pub xs: String,
    /// Small.
    pub sm: String,
    /// Medium / base.
    pub md: String,
    /// Large.
    pub lg: String,
    /// Extra-large.
    pub xl: String,
    /// Double-extra-large.
    pub xxl: String,
}

/// Font-weight scale.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TypographyWeights {
    /// Regular weight (typically 400).
    pub regular: u16,
    /// Medium weight (typically 500).
    pub medium: u16,
    /// Semibold weight (typically 600).
    pub semibold: u16,
    /// Bold weight (typically 700).
    pub bold: u16,
}
