//! Card-component design tokens.

use serde::{Deserialize, Serialize};

/// Card-component design tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardConfig {
    /// Card border-radius tokens.
    pub radius: CardRadius,
    /// Card padding tokens.
    pub padding: CardPadding,
    /// Card-gradient tokens.
    pub gradient: CardGradient,
}

/// Card border-radius tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardRadius {
    /// Default rounded radius.
    pub default: String,
    /// Cut / asymmetric corner radius.
    pub cut: String,
}

/// Card padding tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardPadding {
    /// Small.
    pub sm: String,
    /// Medium / base.
    pub md: String,
    /// Large.
    pub lg: String,
}

/// Card-gradient stop tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardGradient {
    /// Gradient start colour.
    pub start: String,
    /// Gradient mid colour.
    pub mid: String,
    /// Gradient end colour.
    pub end: String,
}
