//! The closed core declaration table behind `admit_extension`: an install
//! extension declaring handler verification, a reserved base path or a body
//! limit above the server default is refused, a listed core declaration is
//! admitted only under its own extension id, at its own base path, in its own
//! route group and with exactly its listed protection shape, and a
//! declaration naming a row with any other shape is refused.

use axum::Router;
use systemprompt_api::services::middleware::{
    CoreDeclaration, CoreDeclarer, CoreShapeField, DEFAULT_BODY_LIMIT_BYTES, MountError,
    admit_extension,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionRouter, RESERVED_PATHS, RouteGroup,
};
use systemprompt_models::modules::ApiPaths;

pub(super) fn row(entry: CoreDeclaration) -> ExtensionRouter {
    let declared = ExtensionRouter::new(
        Router::new(),
        entry.base_path(),
        entry.policy(),
        entry.context(),
    )
    .in_group(entry.group())
    .with_rate_limit(entry.rate_limit());
    match entry.body_limit() {
        Some(bytes) => declared.with_body_limit(bytes),
        None => declared,
    }
}

pub(super) fn extension_id(entry: CoreDeclaration) -> &'static str {
    match entry.declarer() {
        CoreDeclarer::Extension(id) => id,
        CoreDeclarer::Server => panic!("{entry:?} is declared by the server"),
    }
}

fn extension_rows() -> impl Iterator<Item = CoreDeclaration> {
    CoreDeclaration::ALL
        .iter()
        .copied()
        .filter(|entry| matches!(entry.declarer(), CoreDeclarer::Extension(_)))
        .filter(|entry| entry.group() != RouteGroup::Gateway)
}

#[test]
fn only_the_agent_proxy_row_pins_a_body_limit() {
    assert_eq!(
        CoreDeclaration::AgentProxy.body_limit(),
        Some(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES)
    );
    for entry in CoreDeclaration::ALL.iter().skip(1) {
        assert_eq!(entry.body_limit(), None, "{entry:?}");
    }
}

#[test]
fn every_listed_extension_declaration_is_admitted_in_its_own_shape() {
    for entry in extension_rows() {
        assert!(
            admit_extension(extension_id(entry), &row(entry)).is_ok(),
            "{entry:?} must be admitted"
        );
    }
}

#[test]
fn an_install_extension_declaring_handler_verification_is_refused() {
    assert!(matches!(
        admit_extension("install", &row(CoreDeclaration::Slack)),
        Err(MountError::ExtensionHandlerVerified { .. })
    ));
}

#[test]
fn an_install_extension_under_a_reserved_prefix_is_refused() {
    for prefix in RESERVED_PATHS {
        let declared = ExtensionRouter::new(
            Router::new(),
            prefix,
            AuthzPolicy::admin(),
            ContextKind::User,
        );
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
    assert!(matches!(
        admit_extension("install", &row(CoreDeclaration::NotificationsAdmin)),
        Err(MountError::ExtensionReservedPath { .. })
    ));
}

#[test]
fn an_install_extension_with_an_oversized_body_is_refused() {
    let elsewhere = ExtensionRouter::new(
        Router::new(),
        "/api/v1/install",
        AuthzPolicy::user(),
        ContextKind::User,
    )
    .with_body_limit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES);
    assert!(matches!(
        admit_extension("install", &elsewhere),
        Err(MountError::ExtensionBodyLimitTooLarge { .. })
    ));
    assert!(matches!(
        admit_extension("install", &row(CoreDeclaration::AgentProxy)),
        Err(MountError::ExtensionReservedPath { .. })
    ));
}

#[test]
fn a_listed_id_at_another_path_or_group_is_refused_as_drift() {
    let other_path = ExtensionRouter::new(
        Router::new(),
        "/api/v1/slack-other",
        CoreDeclaration::Slack.policy(),
        ContextKind::HandlerResolved,
    )
    .in_group(RouteGroup::Messaging)
    .with_rate_limit(CoreDeclaration::Slack.rate_limit());
    assert!(matches!(
        admit_extension("slack", &other_path),
        Err(MountError::CoreDeclarationMismatch {
            extension: "slack",
            field: CoreShapeField::BasePath,
            ..
        })
    ));

    let other_group = row(CoreDeclaration::Slack).in_group(RouteGroup::Extensions);
    assert!(matches!(
        admit_extension("slack", &other_group),
        Err(MountError::CoreDeclarationMismatch {
            field: CoreShapeField::Group,
            ..
        })
    ));

    let admin_elsewhere = row(CoreDeclaration::NotificationsAdmin).in_group(RouteGroup::Extensions);
    assert!(matches!(
        admit_extension("notifications-http", &admin_elsewhere),
        Err(MountError::CoreDeclarationMismatch {
            field: CoreShapeField::Group,
            ..
        })
    ));
}

#[test]
fn a_listed_row_declared_with_another_policy_is_refused() {
    let wrong_verifier = ExtensionRouter::new(
        Router::new(),
        ApiPaths::TEAMS_BASE,
        CoreDeclaration::Slack.policy(),
        ContextKind::HandlerResolved,
    )
    .in_group(RouteGroup::Messaging)
    .with_rate_limit(CoreDeclaration::Teams.rate_limit());
    assert!(matches!(
        admit_extension("teams", &wrong_verifier),
        Err(MountError::CoreDeclarationMismatch {
            field: CoreShapeField::Policy,
            ..
        })
    ));

    for policy in [
        AuthzPolicy::user(),
        AuthzPolicy::authenticated(),
        AuthzPolicy::public(),
    ] {
        let widened = ExtensionRouter::new(
            Router::new(),
            ApiPaths::ADMIN_NOTIFICATIONS,
            policy,
            ContextKind::User,
        )
        .in_group(RouteGroup::ContentAndMisc)
        .with_rate_limit(CoreDeclaration::NotificationsAdmin.rate_limit());
        assert!(
            matches!(
                admit_extension("notifications-http", &widened),
                Err(MountError::CoreDeclarationMismatch {
                    field: CoreShapeField::Policy,
                    ..
                })
            ),
            "{policy:?} at the reserved prefix must be refused"
        );
    }
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
