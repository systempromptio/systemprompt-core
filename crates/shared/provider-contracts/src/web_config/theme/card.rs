//! Card-component design tokens.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardConfig {
    pub radius: CardRadius,
    pub padding: CardPadding,
    pub gradient: CardGradient,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardRadius {
    pub default: String,
    pub cut: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardPadding {
    pub sm: String,
    pub md: String,
    pub lg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardGradient {
    pub start: String,
    pub mid: String,
    pub end: String,
}
