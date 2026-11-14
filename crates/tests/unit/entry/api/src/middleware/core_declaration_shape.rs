//! Shape drift: a declaration that names a core declaration row (extension
//! id, base path and route group) but differs from the row's pinned
//! protection shape in any field is refused at boot, and the refusal names
//! the field.

use axum::Router;
use systemprompt_api::services::middleware::{
    CoreDeclaration, CoreShapeField, MountError, admit_extension,
};
use systemprompt_extension::prelude::FrameOptions;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionRouter, ProtectionLayer, RateBudget, RateLimit, RouteGroup,
};
use systemprompt_models::modules::ApiPaths;

use super::core_declaration::row;

fn refused_for(extension: &'static str, declared: &ExtensionRouter) -> Option<CoreShapeField> {
    match admit_extension(extension, declared) {
        Err(MountError::CoreDeclarationMismatch {
            extension: refused,
            base_path,
            field,
        }) if refused == extension && base_path == declared.base_path => Some(field),
        _ => None,
    }
}

#[test]
fn a_listed_declaration_with_another_budget_is_refused() {
    let declared = row(CoreDeclaration::AgentRuns)
        .with_rate_limit(RateLimit::new("agent_runs", RateBudget::Agents));
    assert_eq!(
        refused_for("agent_runs_api", &declared),
        Some(CoreShapeField::RateLimit)
    );
}

#[test]
fn a_listed_declaration_without_its_rate_limit_is_refused() {
    let declared = ExtensionRouter::new(
        Router::new(),
        ApiPaths::AGENT_RUNS,
        AuthzPolicy::user(),
        ContextKind::User,
    )
    .in_group(RouteGroup::Agent);
    assert_eq!(
        refused_for("agent_runs_api", &declared),
        Some(CoreShapeField::RateLimit)
    );
}

#[test]
fn a_listed_declaration_with_a_bearer_window_it_does_not_list_is_refused() {
    let declared = row(CoreDeclaration::Notifications).with_rate_limit(
        RateLimit::new("notifications", RateBudget::Notifications).with_bearer_window(),
    );
    assert_eq!(
        refused_for("notifications-http", &declared),
        Some(CoreShapeField::RateLimit)
    );
}

#[test]
fn a_listed_declaration_with_another_layer_set_is_refused() {
    let declared = row(CoreDeclaration::NotificationsAdmin).with_layer(ProtectionLayer::SiteAuth);
    assert_eq!(
        refused_for("notifications-http", &declared),
        Some(CoreShapeField::Layers)
    );
}

#[test]
fn a_listed_declaration_with_other_frame_options_is_refused() {
    let declared = row(CoreDeclaration::Slack).with_frame_options(FrameOptions::AllowAll);
    assert_eq!(
        refused_for("slack", &declared),
        Some(CoreShapeField::FrameOptions)
    );
}

#[test]
fn a_listed_declaration_with_another_context_or_body_limit_is_refused() {
    let context = ExtensionRouter::new(
        Router::new(),
        ApiPaths::NOTIFICATIONS,
        AuthzPolicy::user(),
        ContextKind::A2a,
    )
    .in_group(RouteGroup::ContentAndMisc)
    .with_rate_limit(CoreDeclaration::Notifications.rate_limit());
    assert_eq!(
        refused_for("notifications-http", &context),
        Some(CoreShapeField::Context)
    );

    let body = row(CoreDeclaration::AgentRuns).with_body_limit(1024);
    assert_eq!(
        refused_for("agent_runs_api", &body),
        Some(CoreShapeField::BodyLimit)
    );
}

#[test]
fn a_core_declaration_moved_to_another_path_is_refused() {
    let moved = ExtensionRouter::new(
        Router::new(),
        "/api/v1/agent-runs-v2",
        AuthzPolicy::user(),
        ContextKind::User,
    )
    .in_group(RouteGroup::Agent)
    .with_rate_limit(CoreDeclaration::AgentRuns.rate_limit());
    assert_eq!(
        refused_for("agent_runs_api", &moved),
        Some(CoreShapeField::BasePath)
    );
}

#[test]
fn a_core_declaration_moved_to_another_group_is_refused() {
    let regrouped = row(CoreDeclaration::AgentRuns).in_group(RouteGroup::ContentAndMisc);
    assert_eq!(
        refused_for("agent_runs_api", &regrouped),
        Some(CoreShapeField::Group)
    );
}

#[test]
fn an_install_id_is_not_judged_against_core_rows() {
    let declared = row(CoreDeclaration::AgentRuns);
    assert_eq!(refused_for("install", &declared), None);
    assert!(admit_extension("install", &declared).is_ok());
}
