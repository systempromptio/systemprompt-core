//! Service category classification.

use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceCategory {
    Core,
    Agent,
    Mcp,
    Meta,
}

impl ServiceCategory {
    pub const fn base_path(&self) -> &'static str {
        match self {
            Self::Core => "/api/v1/core",
            Self::Agent => "/api/v1/agents",
            Self::Mcp => "/api/v1/mcp",
            Self::Meta => "/",
        }
    }

    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Core => "Core",
            Self::Agent => "Agent",
            Self::Mcp => "MCP",
            Self::Meta => "Meta",
        }
    }

    pub fn mount_path(&self, module_name: &str) -> String {
        if module_name.is_empty() {
            self.base_path().to_string()
        } else {
            match self {
                Self::Meta => {
                    format!("/{module_name}")
                },
                Self::Core | Self::Agent | Self::Mcp => {
                    format!("{}/{}", self.base_path(), module_name)
                },
            }
        }
    }

    pub fn matches_path(&self, path: &str) -> bool {
        let base = self.base_path();
        if base == "/" {
            path == "/" || path.starts_with("/.well-known") || path.starts_with("/api/v1/meta")
        } else {
            path.starts_with(base)
        }
    }

    pub const fn all() -> &'static [Self] {
        &[Self::Core, Self::Agent, Self::Mcp, Self::Meta]
    }

    pub fn from_path(path: &str) -> Option<Self> {
        for category in &[Self::Core, Self::Agent, Self::Mcp] {
            if category.matches_path(path) {
                return Some(*category);
            }
        }
        if Self::Meta.matches_path(path) {
            Some(Self::Meta)
        } else {
            None
        }
    }
}
