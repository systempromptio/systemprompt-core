//! The rows of the closed core declaration table, and the analytics and
//! content rows: each is admitted only in its own shape under its own id,
//! one owner may list two shapes at one base path, engagement is admitted at
//! `/track/engagement` and nowhere else, and a row whose owner is not linked
//! changes nothing for any other declarer.

use axum::Router;
use systemprompt_api::services::middleware::{
    Allowance, CoreDeclaration, CoreDeclarer, CoreShapeField, MountError, admit_extension,
    extension_served,
};
use systemprompt_extension::registry::WEB_ROOT_BASE_PATH;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionRouter, HandlerVerifier, RouteGroup,
};
use systemprompt_models::modules::ApiPaths;

use super::core_declaration::row;

const fn ext(id: &'static str) -> CoreDeclarer {
    CoreDeclarer::Extension(id)
}

#[test]
fn the_table_lists_exactly_this_waves_core_declarations() {
    let rows: Vec<_> = CoreDeclaration::ALL
        .iter()
        .map(|entry| {
            (
                entry.declarer(),
                entry.base_path(),
                entry.group(),
                entry.allowance(),
            )
        })
        .collect();
    let misc = RouteGroup::ContentAndMisc;
    assert_eq!(
        rows,
        vec![
            (
                CoreDeclarer::Server,
                ApiPaths::AGENTS_BASE,
                RouteGroup::Agent,
                Allowance::BodyLimit,
            ),
            (
                ext("agent_runs_api"),
                ApiPaths::AGENT_RUNS,
                RouteGroup::Agent,
                Allowance::Plain,
            ),
            (
                ext("slack"),
                ApiPaths::SLACK_BASE,
                RouteGroup::Messaging,
                Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
            ),
            (
                ext("teams"),
                ApiPaths::TEAMS_BASE,
                RouteGroup::Messaging,
                Allowance::HandlerVerified(HandlerVerifier::TeamsBotToken),
            ),
            (
                ext("notifications-http"),
                ApiPaths::NOTIFICATIONS,
                misc,
                Allowance::Plain,
            ),
            (
                ext("notifications-http"),
                ApiPaths::ADMIN_NOTIFICATIONS,
                misc,
                Allowance::ReservedPrefix,
            ),
            (
                ext("notifications-http"),
                ApiPaths::BRIDGE_NOTIFICATIONS,
                RouteGroup::Gateway,
                Allowance::HandlerVerified(HandlerVerifier::GatewayCredential),
            ),
            (
                ext("analytics-http"),
                ApiPaths::ANALYTICS_BASE,
                misc,
                Allowance::ReservedPrefix,
            ),
            (
                ext("analytics-http"),
                ApiPaths::TRACK_ENGAGEMENT,
                misc,
                Allowance::WebPath,
            ),
            (
                ext("content-http"),
                ApiPaths::CONTENT_BASE,
                misc,
                Allowance::ReservedPrefix,
            ),
            (
                ext("content-http"),
                ApiPaths::CONTENT_BASE,
                misc,
                Allowance::ReservedPrefix,
            ),
            (
                ext("content-http"),
                WEB_ROOT_BASE_PATH,
                misc,
                Allowance::Plain,
            ),
        ]
    );
}

#[test]
fn the_analytics_and_content_rows_pin_todays_shapes() {
    let content = systemprompt_extension::RateLimit::new(
        "content",
        systemprompt_extension::RateBudget::Content,
    );
    for (entry, policy, context) in [
        (
            CoreDeclaration::Analytics,
            AuthzPolicy::admin(),
            ContextKind::User,
        ),
        (
            CoreDeclaration::Engagement,
            AuthzPolicy::public(),
            ContextKind::Public,
        ),
        (
            CoreDeclaration::ContentPublic,
            AuthzPolicy::public(),
            ContextKind::Public,
        ),
        (
            CoreDeclaration::ContentAuthenticated,
            AuthzPolicy::user(),
            ContextKind::User,
        ),
        (
            CoreDeclaration::ContentRedirect,
            AuthzPolicy::public(),
            ContextKind::Public,
        ),
    ] {
        assert_eq!(entry.policy().allowed(), policy.allowed(), "{entry:?}");
        assert_eq!(entry.policy().handler_verifier(), None, "{entry:?}");
        assert_eq!(entry.context(), context, "{entry:?}");
        assert_eq!(entry.rate_limit(), content, "{entry:?}");
        assert!(!entry.rate_limit().bearer_window, "{entry:?}");
        assert_eq!(entry.body_limit(), None, "{entry:?}");
    }
}

#[test]
fn one_owner_may_declare_both_shapes_at_the_content_base_path() {
    for entry in [
        CoreDeclaration::ContentPublic,
        CoreDeclaration::ContentAuthenticated,
    ] {
        assert!(
            admit_extension("content-http", &row(entry)).is_ok(),
            "{entry:?} must be admitted"
        );
    }

    let admin = ExtensionRouter::new(
        Router::new(),
        ApiPaths::CONTENT_BASE,
        AuthzPolicy::admin(),
        ContextKind::User,
    )
    .in_group(RouteGroup::ContentAndMisc)
    .with_rate_limit(CoreDeclaration::ContentPublic.rate_limit());
    assert!(matches!(
        admit_extension("content-http", &admin),
        Err(MountError::CoreDeclarationMismatch {
            extension: "content-http",
            field: CoreShapeField::Policy,
            ..
        })
    ));
}

#[test]
fn a_capability_owner_declaring_an_unlisted_router_is_refused() {
    let elsewhere = ExtensionRouter::new(
        Router::new(),
        "/api/v1/analytics-extra",
        AuthzPolicy::admin(),
        ContextKind::User,
    )
    .in_group(RouteGroup::ContentAndMisc)
    .with_rate_limit(CoreDeclaration::Analytics.rate_limit());
    assert!(matches!(
        admit_extension("analytics-http", &elsewhere),
        Err(MountError::CoreDeclarationMismatch {
            field: CoreShapeField::BasePath,
            ..
        })
    ));

    let unlimited = ExtensionRouter::new(
        Router::new(),
        ApiPaths::TRACK_ENGAGEMENT,
        AuthzPolicy::public(),
        ContextKind::Public,
    )
    .in_group(RouteGroup::ContentAndMisc);
    assert!(matches!(
        admit_extension("analytics-http", &unlimited),
        Err(MountError::CoreDeclarationMismatch {
            field: CoreShapeField::RateLimit,
            ..
        })
    ));

    let at_root = ExtensionRouter::new(
        Router::new(),
        WEB_ROOT_BASE_PATH,
        AuthzPolicy::public(),
        ContextKind::Public,
    )
    .in_group(RouteGroup::ContentAndMisc)
    .with_rate_limit(CoreDeclaration::Engagement.rate_limit());
    assert!(
        matches!(
            admit_extension("analytics-http", &at_root),
            Err(MountError::CoreDeclarationMismatch {
                field: CoreShapeField::BasePath,
                ..
            })
        ),
        "engagement is no longer admitted at the web root"
    );
}

#[test]
fn another_id_at_a_capability_row_gets_neither_the_prefix_nor_core_exposure() {
    assert!(matches!(
        admit_extension("install", &row(CoreDeclaration::ContentPublic)),
        Err(MountError::ExtensionReservedPath { .. })
    ));
    assert!(matches!(
        admit_extension("install", &row(CoreDeclaration::Analytics)),
        Err(MountError::ExtensionReservedPath { .. })
    ));

    let at_root = row(CoreDeclaration::ContentRedirect);
    assert!(admit_extension("install", &at_root).is_ok());
    let without_extensions = |group: RouteGroup| group == RouteGroup::ContentAndMisc;
    assert!(!extension_served("install", &at_root, &without_extensions));
    assert!(extension_served(
        "content-http",
        &at_root,
        &without_extensions
    ));
}
