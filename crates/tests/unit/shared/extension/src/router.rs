use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionContext, ExtensionRouter, ProtectionLayer, RateBudget,
    RateLimit, RouteGroup, RouteProtection, SiteAuthConfig,
};
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_provider_contracts::FrameOptions;

use crate::stub_ctx::StubCtx;

fn declared(policy: AuthzPolicy, context: ContextKind) -> ExtensionRouter {
    ExtensionRouter::new(axum::Router::new(), "/api/v2/thing", policy, context)
}

#[test]
fn a_user_declaration_requires_authentication() {
    let router = declared(AuthzPolicy::user(), ContextKind::User);
    assert!(router.protection.requires_authentication());
    assert_eq!(router.protection.context, ContextKind::User);
}

#[test]
fn a_public_declaration_admits_anonymous_callers() {
    let router = declared(AuthzPolicy::public(), ContextKind::Public);
    assert!(!router.protection.requires_authentication());
}

#[test]
fn a_declaration_starts_with_no_limits_layers_or_frame_override() {
    let protection = declared(AuthzPolicy::user(), ContextKind::User).protection;
    assert!(protection.rate_limit.is_none());
    assert!(protection.body_limit.is_none());
    assert!(protection.layers.is_empty());
    assert!(protection.frame_options.is_none());
}

#[test]
fn builders_record_every_protection_field() {
    let router = declared(AuthzPolicy::authenticated(), ContextKind::A2a)
        .with_rate_limit(RateLimit::new("agents", RateBudget::Agents).with_bearer_window())
        .with_body_limit(4096)
        .with_layer(ProtectionLayer::SiteAuth)
        .with_layer(ProtectionLayer::SiteAuth)
        .with_frame_options(FrameOptions::AllowAll)
        .in_group(RouteGroup::Agent);
    let rate = router.protection.rate_limit.expect("rate limit recorded");
    assert_eq!(rate.scope, "agents");
    assert_eq!(rate.budget, RateBudget::Agents);
    assert!(rate.bearer_window);
    assert_eq!(router.protection.body_limit, Some(4096));
    assert_eq!(router.protection.layers, vec![ProtectionLayer::SiteAuth]);
    assert_eq!(
        router.protection.frame_options,
        Some(FrameOptions::AllowAll)
    );
    assert_eq!(router.group, RouteGroup::Agent);
}

#[test]
fn a_rate_budget_resolves_its_profile_rate() {
    let config = RateLimitsConfig {
        content_per_second: 7,
        gateway_per_second: 11,
        ..RateLimitsConfig::default()
    };
    assert_eq!(RateBudget::Content.per_second(&config), 7);
    assert_eq!(RateBudget::Gateway.per_second(&config), 11);
    assert_eq!(RateBudget::PerSecond(3).per_second(&config), 3);
}

#[test]
fn route_protection_new_takes_the_policy_and_context() {
    let protection = RouteProtection::new(AuthzPolicy::deferred_to_handler(), ContextKind::Mcp);
    assert_eq!(protection.context, ContextKind::Mcp);
    assert!(!protection.requires_authentication());
}

fn never(_ctx: &dyn ExtensionContext) -> bool {
    false
}

#[test]
fn the_enable_predicate_decides_whether_a_declaration_is_enabled() {
    let always = declared(AuthzPolicy::user(), ContextKind::User);
    assert!(always.is_enabled(&StubCtx));
    let disabled = declared(AuthzPolicy::user(), ContextKind::User).enabled_when(never);
    assert!(!disabled.is_enabled(&StubCtx));
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
