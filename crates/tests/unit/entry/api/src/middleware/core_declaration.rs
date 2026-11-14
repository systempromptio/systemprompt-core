//! The closed core declaration table behind `admit_extension` and
//! `extension_served`: an install extension declaring handler verification, a
//! reserved base path or a body limit above the server default is refused, a
//! listed core declaration is admitted only under its own extension id, at
//! its own base path, in its own route group and with exactly its listed
//! protection, and only a listed core declaration is served by its own
//! group's roles alone.

use axum::Router;
use systemprompt_api::services::middleware::{
    Allowance, CoreDeclaration, CoreDeclarer, DEFAULT_BODY_LIMIT_BYTES, MountError,
    admit_extension, extension_served,
};
use systemprompt_api::services::server::routes::role::route_groups;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionRouter, HandlerVerifier, RESERVED_PATHS, RouteGroup,
};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_models::modules::ApiPaths;

fn slack(base_path: &'static str) -> ExtensionRouter {
    ExtensionRouter::new(
        Router::new(),
        base_path,
        AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
        ContextKind::HandlerResolved,
    )
    .in_group(RouteGroup::Messaging)
}

fn notifications_admin(base_path: &'static str, policy: AuthzPolicy) -> ExtensionRouter {
    ExtensionRouter::new(Router::new(), base_path, policy, ContextKind::User)
        .in_group(RouteGroup::ContentAndMisc)
}

#[test]
fn the_table_lists_exactly_this_waves_core_declarations() {
    let groups: Vec<_> = CoreDeclaration::ALL
        .iter()
        .map(|entry| entry.group())
        .collect();
    assert_eq!(
        groups,
        vec![
            RouteGroup::Agent,
            RouteGroup::Agent,
            RouteGroup::Messaging,
            RouteGroup::Messaging,
            RouteGroup::ContentAndMisc,
            RouteGroup::ContentAndMisc,
        ]
    );

    let rows: Vec<_> = CoreDeclaration::ALL
        .iter()
        .map(|entry| (entry.declarer(), entry.base_path(), entry.allowance()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (
                CoreDeclarer::Server,
                ApiPaths::AGENTS_BASE,
                Allowance::BodyLimit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES),
            ),
            (
                CoreDeclarer::Extension("agent_runs_api"),
                ApiPaths::AGENT_RUNS,
                Allowance::Plain,
            ),
            (
                CoreDeclarer::Extension("slack"),
                ApiPaths::SLACK_BASE,
                Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
            ),
            (
                CoreDeclarer::Extension("teams"),
                ApiPaths::TEAMS_BASE,
                Allowance::HandlerVerified(HandlerVerifier::TeamsBotToken),
            ),
            (
                CoreDeclarer::Extension("notifications-http"),
                ApiPaths::NOTIFICATIONS,
                Allowance::Plain,
            ),
            (
                CoreDeclarer::Extension("notifications-http"),
                ApiPaths::ADMIN_NOTIFICATIONS,
                Allowance::ReservedPrefix,
            ),
        ]
    );
}

#[test]
fn an_install_extension_declaring_handler_verification_is_refused() {
    assert!(matches!(
        admit_extension("install", &slack(ApiPaths::SLACK_BASE)),
        Err(MountError::ExtensionHandlerVerified { .. })
    ));
}

#[test]
fn an_install_extension_under_a_reserved_prefix_is_refused() {
    for prefix in RESERVED_PATHS {
        let declared = notifications_admin(prefix, AuthzPolicy::admin());
        assert!(
            matches!(
                admit_extension("install", &declared),
                Err(MountError::ExtensionReservedPath {
                    extension: "install",
                    ..
                })
            ),
            "{prefix} must be refused to an install extension"
        );
    }
    let nested = notifications_admin(ApiPaths::ADMIN_NOTIFICATIONS, AuthzPolicy::admin());
    assert!(matches!(
        admit_extension("install", &nested),
        Err(MountError::ExtensionReservedPath { .. })
    ));
}

#[test]
fn an_install_extension_with_an_oversized_body_is_refused() {
    let declared = ExtensionRouter::new(
        Router::new(),
        ApiPaths::AGENTS_BASE,
        AuthzPolicy::authenticated(),
        ContextKind::User,
    )
    .with_body_limit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES);
    assert!(matches!(
        admit_extension("install", &declared),
        Err(MountError::ExtensionReservedPath { .. })
    ));

    let elsewhere = ExtensionRouter::new(
        Router::new(),
        "/api/v1/install",
        AuthzPolicy::user(),
        ContextKind::User,
    )
    .with_body_limit(DEFAULT_BODY_LIMIT_BYTES + 1);
    assert!(matches!(
        admit_extension("install", &elsewhere),
        Err(MountError::ExtensionBodyLimitTooLarge { .. })
    ));
}

#[test]
fn the_server_body_limit_row_grants_no_extension() {
    let declared = ExtensionRouter::new(
        Router::new(),
        "/api/v1/install",
        AuthzPolicy::user(),
        ContextKind::User,
    )
    .with_body_limit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES);
    assert!(matches!(
        admit_extension("install", &declared),
        Err(MountError::ExtensionBodyLimitTooLarge { .. })
    ));
}

#[test]
fn listed_messaging_declarations_are_admitted_at_their_own_path() {
    assert!(admit_extension("slack", &slack(ApiPaths::SLACK_BASE)).is_ok());

    let teams = ExtensionRouter::new(
        Router::new(),
        ApiPaths::TEAMS_BASE,
        AuthzPolicy::handler_verified(HandlerVerifier::TeamsBotToken),
        ContextKind::HandlerResolved,
    )
    .in_group(RouteGroup::Messaging);
    assert!(admit_extension("teams", &teams).is_ok());
}

#[test]
fn a_listed_id_at_another_path_or_shape_is_refused() {
    assert!(matches!(
        admit_extension("slack", &slack("/api/v1/slack-other")),
        Err(MountError::ExtensionHandlerVerified { .. })
    ));
    assert!(matches!(
        admit_extension("slack", &slack(ApiPaths::TEAMS_BASE)),
        Err(MountError::ExtensionHandlerVerified { .. })
    ));
    assert!(matches!(
        admit_extension("teams", &slack(ApiPaths::TEAMS_BASE)),
        Err(MountError::ExtensionHandlerVerified { .. })
    ));

    let wrong_group = slack(ApiPaths::SLACK_BASE).in_group(RouteGroup::Extensions);
    assert!(matches!(
        admit_extension("slack", &wrong_group),
        Err(MountError::ExtensionHandlerVerified { .. })
    ));
}

#[test]
fn the_notifications_admin_row_admits_only_the_admin_declaration() {
    let listed = notifications_admin(ApiPaths::ADMIN_NOTIFICATIONS, AuthzPolicy::admin());
    assert!(admit_extension("notifications-http", &listed).is_ok());

    for policy in [
        AuthzPolicy::user(),
        AuthzPolicy::authenticated(),
        AuthzPolicy::public(),
    ] {
        let widened = notifications_admin(ApiPaths::ADMIN_NOTIFICATIONS, policy);
        assert!(
            matches!(
                admit_extension("notifications-http", &widened),
                Err(MountError::ExtensionReservedPath { .. })
            ),
            "{policy:?} at the reserved prefix must be refused"
        );
    }

    let other_path = notifications_admin("/api/v1/admin/other", AuthzPolicy::admin());
    assert!(matches!(
        admit_extension("notifications-http", &other_path),
        Err(MountError::ExtensionReservedPath { .. })
    ));

    let other_group = notifications_admin(ApiPaths::ADMIN_NOTIFICATIONS, AuthzPolicy::admin())
        .in_group(RouteGroup::Extensions);
    assert!(matches!(
        admit_extension("notifications-http", &other_group),
        Err(MountError::ExtensionReservedPath { .. })
    ));

    assert!(matches!(
        admit_extension("slack", &listed),
        Err(MountError::ExtensionReservedPath { .. })
    ));
}

#[test]
fn an_unprivileged_install_declaration_is_admitted() {
    let declared = ExtensionRouter::new(
        Router::new(),
        "/api/v1/install",
        AuthzPolicy::user(),
        ContextKind::User,
    )
    .with_body_limit(DEFAULT_BODY_LIMIT_BYTES);
    assert!(admit_extension("install", &declared).is_ok());
}

fn served_on(role: NodeRole, extension: &'static str, declared: &ExtensionRouter) -> bool {
    let groups = route_groups(role);
    extension_served(extension, declared, &|group| groups.contains(&group))
}

fn agent_group(base_path: &'static str) -> ExtensionRouter {
    ExtensionRouter::new(
        Router::new(),
        base_path,
        AuthzPolicy::user(),
        ContextKind::User,
    )
    .in_group(RouteGroup::Agent)
}

const ROLES: [NodeRole; 5] = [
    NodeRole::All,
    NodeRole::Gateway,
    NodeRole::Api,
    NodeRole::Worker,
    NodeRole::Runner,
];

#[test]
fn an_unlisted_agent_group_declaration_keeps_its_base_role_exposure() {
    let declared = agent_group("/api/v1/install");
    for role in ROLES {
        let groups = route_groups(role);
        let base = groups.contains(&RouteGroup::Extensions) && groups.contains(&RouteGroup::Agent);
        assert_eq!(
            served_on(role, "install", &declared),
            base,
            "{role:?} must serve an install extension's agent-group router exactly as before"
        );
    }
    assert!(!served_on(NodeRole::Worker, "install", &declared));
}

#[test]
fn the_listed_agent_runs_declaration_is_served_on_workers() {
    let declared = agent_group(ApiPaths::AGENT_RUNS);
    assert!(served_on(NodeRole::Worker, "agent_runs_api", &declared));
    assert!(served_on(NodeRole::All, "agent_runs_api", &declared));
    assert!(served_on(NodeRole::Api, "agent_runs_api", &declared));
    assert!(!served_on(NodeRole::Gateway, "agent_runs_api", &declared));
    assert!(!served_on(NodeRole::Runner, "agent_runs_api", &declared));
}

#[test]
fn a_listed_id_at_another_path_or_group_gets_no_core_exposure() {
    let other_path = agent_group("/api/v1/agent-runs-extra");
    assert!(!served_on(NodeRole::Worker, "agent_runs_api", &other_path));

    let other_group = agent_group(ApiPaths::AGENT_RUNS).in_group(RouteGroup::McpAndStream);
    assert!(!served_on(NodeRole::Worker, "agent_runs_api", &other_group));

    let other_id = agent_group(ApiPaths::AGENT_RUNS);
    assert!(!served_on(NodeRole::Worker, "install", &other_id));
}

#[test]
fn an_install_extensions_default_group_is_served_by_all_and_api_only() {
    let declared = ExtensionRouter::new(
        Router::new(),
        "/api/v1/install",
        AuthzPolicy::user(),
        ContextKind::User,
    );
    let serving: Vec<_> = ROLES
        .into_iter()
        .filter(|role| served_on(*role, "install", &declared))
        .collect();
    assert_eq!(serving, vec![NodeRole::All, NodeRole::Api]);
}
