#[derive(Debug, Clone, Copy)]
pub struct ExtensionRouterConfig {
    pub base_path: &'static str,
    pub requires_auth: bool,
}

impl ExtensionRouterConfig {
    #[must_use]
    pub const fn new(base_path: &'static str) -> Self {
        Self {
            base_path,
            requires_auth: true,
        }
    }

    #[must_use]
    pub const fn public(base_path: &'static str) -> Self {
        Self {
            base_path,
            requires_auth: false,
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
}

impl ExtensionRouter {
    #[must_use]
    pub const fn new(router: axum::Router, base_path: &'static str) -> Self {
        Self {
            router,
            base_path,
            requires_auth: true,
        }
    }

    #[must_use]
    pub const fn public(router: axum::Router, base_path: &'static str) -> Self {
        Self {
            router,
            base_path,
            requires_auth: false,
        }
    }

    #[must_use]
    pub const fn config(&self) -> ExtensionRouterConfig {
        ExtensionRouterConfig {
            base_path: self.base_path,
            requires_auth: self.requires_auth,
        }
    }
}
