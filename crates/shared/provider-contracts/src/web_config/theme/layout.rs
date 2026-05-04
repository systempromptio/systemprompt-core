//! Layout dimensions: header height, sidebar widths, content max-width.

use serde::{Deserialize, Serialize};

/// Layout dimensions: header height, sidebar widths, ...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Top-header height.
    #[serde(rename = "headerHeight")]
    pub header_height: String,
    /// Left-sidebar configuration.
    #[serde(rename = "sidebarLeft")]
    pub sidebar_left: SidebarConfig,
    /// Right-sidebar configuration.
    #[serde(rename = "sidebarRight")]
    pub sidebar_right: SidebarConfig,
    /// Top-navigation height.
    #[serde(rename = "navHeight")]
    pub nav_height: String,
    /// Maximum content width.
    #[serde(rename = "contentMaxWidth")]
    pub content_max_width: String,
}

/// One sidebar's width configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarConfig {
    /// Default width.
    pub width: String,
    /// Minimum width.
    #[serde(rename = "minWidth")]
    pub min_width: String,
    /// Maximum width.
    #[serde(rename = "maxWidth")]
    pub max_width: String,
}
