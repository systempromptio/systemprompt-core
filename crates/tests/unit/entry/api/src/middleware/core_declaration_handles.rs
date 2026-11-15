//! Kernel handles of the closed core declaration table: only the `Analytics`
//! row is handed one (the analytics event broadcaster), and only a
//! declaration from `analytics-http` that matches that row in full receives
//! it. Another id at the same base path, group and shape, and an
//! `analytics-http` declaration that drifts from the row, receive nothing.

use systemprompt_api::services::middleware::{CoreDeclaration, KernelHandle, kernel_handles_for};
use systemprompt_extension::{AuthzPolicy, RouteGroup};

use super::core_declaration::row;

#[test]
fn only_the_analytics_row_is_handed_a_kernel_handle() {
    let handed: Vec<_> = CoreDeclaration::ALL
        .iter()
        .filter(|entry| !entry.kernel_handles().is_empty())
        .map(|entry| (*entry, entry.kernel_handles()))
        .collect();
    assert_eq!(
        handed,
        vec![(
            CoreDeclaration::Analytics,
            &[KernelHandle::AnalyticsBroadcaster][..]
        )]
    );
}

#[test]
fn the_owner_declaring_the_analytics_row_in_full_receives_the_broadcaster() {
    assert_eq!(
        kernel_handles_for("analytics-http", &row(CoreDeclaration::Analytics)),
        &[KernelHandle::AnalyticsBroadcaster]
    );
    assert!(kernel_handles_for("analytics-http", &row(CoreDeclaration::Engagement)).is_empty());
}

#[test]
fn another_id_declaring_the_analytics_rows_shape_receives_nothing() {
    for id in ["install", "content-http", "notifications-http", "analytics"] {
        assert!(
            kernel_handles_for(id, &row(CoreDeclaration::Analytics)).is_empty(),
            "{id}"
        );
    }
}

#[test]
fn an_analytics_declaration_that_drifts_from_its_row_receives_nothing() {
    let mut drifted = row(CoreDeclaration::Analytics);
    drifted.protection.policy = AuthzPolicy::user();
    assert!(kernel_handles_for("analytics-http", &drifted).is_empty());

    let other_group = row(CoreDeclaration::Analytics).in_group(RouteGroup::Extensions);
    assert!(kernel_handles_for("analytics-http", &other_group).is_empty());
}
