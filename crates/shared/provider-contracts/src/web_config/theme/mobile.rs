//! Mobile-breakpoint overrides for spacing, typography, layout, and cards.

use serde::{Deserialize, Serialize};

use super::card::CardPadding;
use super::scale::SpacingConfig;
use super::typography::TypographySizes;

/// Mobile-breakpoint design tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileConfig {
    /// Mobile spacing scale.
    pub spacing: SpacingConfig,
    /// Mobile typography overrides.
    pub typography: MobileTypography,
    /// Mobile layout dimensions.
    pub layout: MobileLayout,
    /// Mobile card overrides.
    pub card: MobileCardConfig,
}

/// Mobile-only typography overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileTypography {
    /// Font sizes scale at mobile breakpoint.
    pub sizes: TypographySizes,
}

/// Mobile-only layout dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileLayout {
    /// Mobile header height.
    #[serde(rename = "headerHeight")]
    pub header_height: String,
    /// Mobile nav height.
    #[serde(rename = "navHeight")]
    pub nav_height: String,
}

/// Mobile-only card overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileCardConfig {
    /// Mobile card padding tokens.
    pub padding: CardPadding,
}
