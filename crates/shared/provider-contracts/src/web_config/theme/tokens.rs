//! Cross-cutting design tokens: shadows, animation, z-index, touch targets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowsConfig {
    pub light: ShadowSet,
    pub dark: ShadowSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowSet {
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub accent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    pub fast: String,
    pub normal: String,
    pub slow: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ZIndexConfig {
    pub base: u32,
    pub content: u32,
    pub navigation: u32,
    pub modal: u32,
    pub tooltip: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchTargetsConfig {
    pub default: String,
    pub sm: String,
    pub lg: String,
}
