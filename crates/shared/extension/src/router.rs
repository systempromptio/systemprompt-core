//! Router and site-auth configuration value types.

/// Mounting configuration for an extension's router.
#[derive(Debug, Clone, Copy)]
pub struct ExtensionRouterConfig {
    /// Mount path (must start with `/api/`).
    pub base_path: &'static str,
    /// Whether the router requires an authenticated request.
    pub requires_auth: bool,
}

impl ExtensionRouterConfig {
    /// Constructs an authenticated router configuration.
    #[must_use]
    pub const fn new(base_path: &'static str) -> Self {
        Self {
            base_path,
            requires_auth: true,
        }
    }

    /// Constructs a public (unauthenticated) router configuration.
    #[must_use]
    pub const fn public(base_path: &'static str) -> Self {
        Self {
            base_path,
            requires_auth: false,
        }
    }
}

/// Site-level authentication configuration that an extension can declare
/// to integrate with the host's auth middleware.
#[derive(Debug, Clone, Copy)]
pub struct SiteAuthConfig {
    /// Login route the host should redirect unauthenticated requests to.
    pub login_path: &'static str,
    /// Path prefixes that require authentication.
    pub protected_prefixes: &'static [&'static str],
    /// Path prefixes that are publicly accessible.
    pub public_prefixes: &'static [&'static str],
    /// OAuth scope required to access protected paths.
    pub required_scope: &'static str,
}

/// Materialised router contributed by an extension, ready for the host to
/// mount.
#[derive(Debug, Clone)]
pub struct ExtensionRouter {
    /// The axum router itself.
    pub router: axum::Router,
    /// Mount path (must start with `/api/`).
    pub base_path: &'static str,
    /// Whether the router requires an authenticated request.
    pub requires_auth: bool,
}

impl ExtensionRouter {
    /// Constructs an authenticated router mounted at `base_path`.
    #[must_use]
    pub const fn new(router: axum::Router, base_path: &'static str) -> Self {
        Self {
            router,
            base_path,
            requires_auth: true,
        }
    }

    /// Constructs a public (unauthenticated) router mounted at
    /// `base_path`.
    #[must_use]
    pub const fn public(router: axum::Router, base_path: &'static str) -> Self {
        Self {
            router,
            base_path,
            requires_auth: false,
        }
    }

    /// Returns the mounting [`ExtensionRouterConfig`] derived from this
    /// router instance.
    #[must_use]
    pub const fn config(&self) -> ExtensionRouterConfig {
        ExtensionRouterConfig {
            base_path: self.base_path,
            requires_auth: self.requires_auth,
        }
    }
}
