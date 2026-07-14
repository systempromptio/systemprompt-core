//! Router and site-auth configuration value types.

use crate::frame_options::FrameOptions;

#[derive(Debug, Clone, Copy)]
pub struct ExtensionRouterConfig {
    pub base_path: &'static str,
    pub requires_auth: bool,
    pub frame_options: Option<FrameOptions>,
}

impl ExtensionRouterConfig {
    #[must_use]
    pub const fn new(base_path: &'static str) -> Self {
        Self {
            base_path,
            requires_auth: true,
            frame_options: None,
        }
    }

    #[must_use]
    pub const fn public(base_path: &'static str) -> Self {
        Self {
            base_path,
            requires_auth: false,
            frame_options: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SiteAuthConfig {
    pub login_path: &'static str,
    pub protected_prefixes: &'static [&'static str],
    pub public_prefixes: &'static [&'static str],
    pub required_scope: &'static str,
}

#[derive(Debug, Clone)]
pub struct ExtensionRouter {
    pub router: axum::Router,
    pub base_path: &'static str,
    pub requires_auth: bool,
    pub frame_options: Option<FrameOptions>,
}

impl ExtensionRouter {
    #[must_use]
    pub const fn new(router: axum::Router, base_path: &'static str) -> Self {
        Self {
            router,
            base_path,
            requires_auth: true,
            frame_options: None,
        }
    }

    #[must_use]
    pub const fn public(router: axum::Router, base_path: &'static str) -> Self {
        Self {
            router,
            base_path,
            requires_auth: false,
            frame_options: None,
        }
    }

    #[must_use]
    pub const fn with_frame_options(mut self, frame_options: FrameOptions) -> Self {
        self.frame_options = Some(frame_options);
        self
    }

    #[must_use]
    pub const fn config(&self) -> ExtensionRouterConfig {
        ExtensionRouterConfig {
            base_path: self.base_path,
            requires_auth: self.requires_auth,
            frame_options: self.frame_options,
        }
    }
}
