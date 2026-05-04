//! Mobile-breakpoint overrides for spacing, typography, layout, and cards.

use serde::{Deserialize, Serialize};

use super::card::CardPadding;
use super::scale::SpacingConfig;
use super::typography::TypographySizes;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileConfig {
    pub spacing: SpacingConfig,
    pub typography: MobileTypography,
    pub layout: MobileLayout,
    pub card: MobileCardConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileTypography {
    pub sizes: TypographySizes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileLayout {
    #[serde(rename = "headerHeight")]
    pub header_height: String,
    #[serde(rename = "navHeight")]
    pub nav_height: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileCardConfig {
    pub padding: CardPadding,
}
