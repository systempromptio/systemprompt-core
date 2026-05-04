//! Layout dimensions: header height, sidebar widths, content max-width.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    #[serde(rename = "headerHeight")]
    pub header_height: String,
    #[serde(rename = "sidebarLeft")]
    pub sidebar_left: SidebarConfig,
    #[serde(rename = "sidebarRight")]
    pub sidebar_right: SidebarConfig,
    #[serde(rename = "navHeight")]
    pub nav_height: String,
    #[serde(rename = "contentMaxWidth")]
    pub content_max_width: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarConfig {
    pub width: String,
    #[serde(rename = "minWidth")]
    pub min_width: String,
    #[serde(rename = "maxWidth")]
    pub max_width: String,
}
