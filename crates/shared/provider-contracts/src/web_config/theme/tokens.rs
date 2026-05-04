//! Cross-cutting design tokens: shadows, animation, z-index, touch targets.

use serde::{Deserialize, Serialize};

/// Light / dark shadow sets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowsConfig {
    /// Light-mode shadows.
    pub light: ShadowSet,
    /// Dark-mode shadows.
    pub dark: ShadowSet,
}

/// One shadow set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowSet {
    /// Small elevation.
    pub sm: String,
    /// Medium elevation.
    pub md: String,
    /// Large elevation.
    pub lg: String,
    /// Brand-accent shadow.
    pub accent: String,
}

/// Animation timing tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    /// Fast animation duration.
    pub fast: String,
    /// Normal animation duration.
    pub normal: String,
    /// Slow animation duration.
    pub slow: String,
}

/// `z-index` token stack.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ZIndexConfig {
    /// Base layer.
    pub base: u32,
    /// Page-content layer.
    pub content: u32,
    /// Top navigation layer.
    pub navigation: u32,
    /// Modal-overlay layer.
    pub modal: u32,
    /// Tooltip layer.
    pub tooltip: u32,
}

/// Touch-target sizing tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchTargetsConfig {
    /// Default touch-target size.
    pub default: String,
    /// Small touch-target size.
    pub sm: String,
    /// Large touch-target size.
    pub lg: String,
}
