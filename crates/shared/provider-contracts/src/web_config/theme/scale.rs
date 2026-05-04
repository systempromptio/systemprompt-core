//! Spacing and border-radius scales.

use serde::{Deserialize, Serialize};

/// Spacing scale (xs..xxl).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacingConfig {
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

/// Border-radius scale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadiusConfig {
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
    /// Fully-rounded (pill).
    pub round: String,
}
