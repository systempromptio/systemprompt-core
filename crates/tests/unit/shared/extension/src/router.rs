use systemprompt_extension::{ExtensionRouterConfig, SiteAuthConfig};

#[test]
fn router_config_new_requires_auth() {
    let config = ExtensionRouterConfig::new("/api/v1/custom");
    assert_eq!(config.base_path, "/api/v1/custom");
    assert!(config.requires_auth);
}

#[test]
fn router_config_public_does_not_require_auth() {
    let config = ExtensionRouterConfig::public("/api/v1/public");
    assert_eq!(config.base_path, "/api/v1/public");
    assert!(!config.requires_auth);
}

#[test]
fn router_config_debug_format() {
    let config = ExtensionRouterConfig::new("/api/v1/debug");
    let debug = format!("{config:?}");
    assert!(debug.contains("/api/v1/debug"));
}

#[test]
fn site_auth_config_fields() {
    let auth = SiteAuthConfig {
        login_path: "/login",
        protected_prefixes: &["/dashboard", "/admin"],
        public_prefixes: &["/public", "/assets"],
        required_scope: "site:access",
    };
    assert_eq!(auth.login_path, "/login");
    assert_eq!(auth.protected_prefixes.len(), 2);
    assert_eq!(auth.public_prefixes.len(), 2);
    assert_eq!(auth.required_scope, "site:access");
}

#[test]
fn site_auth_config_debug_format() {
    let auth = SiteAuthConfig {
        login_path: "/signin",
        protected_prefixes: &[],
        public_prefixes: &[],
        required_scope: "read",
    };
    let debug = format!("{auth:?}");
    assert!(debug.contains("/signin"));
}
