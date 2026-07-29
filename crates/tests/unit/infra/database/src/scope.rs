//! The connection-scope types: setting-key validation, `RequestScope`
//! semantics, and the empty-registry fast path.

use std::sync::Arc;

use systemprompt_database::RequestScope;
use systemprompt_database::scope::{
    ConnectionScopeProvider, ScopeError, ScopeSetting, SharedScopeProvider,
    discover_scope_providers, scope_providers,
};

#[test]
fn a_dotted_custom_guc_key_is_accepted() {
    let setting = ScopeSetting::new("app.current_org", "org_1").expect("valid key");
    assert_eq!(setting.key(), "app.current_org");
    assert_eq!(setting.value(), "org_1");
}

#[test]
fn injection_shaped_and_malformed_keys_are_rejected() {
    for key in [
        "app.x; DROP TABLE users",
        "app.\"quoted\"",
        "current_org",
        "app.",
        ".org",
        "app.current org",
        "app.1st",
        "1app.org",
        "",
    ] {
        let result = ScopeSetting::new(key, "v");
        assert!(
            matches!(result, Err(ScopeError::InvalidKey { .. })),
            "key {key:?} must be rejected"
        );
    }
}

#[test]
fn a_hostile_value_is_carried_verbatim_for_binding() {
    // Values are bound as parameters, never interpolated, so any string is a
    // legal value.
    let setting = ScopeSetting::new("app.current_org", "'; DROP TABLE users; --").expect("valid");
    assert_eq!(setting.value(), "'; DROP TABLE users; --");
}

#[test]
fn request_scope_insert_replaces_and_iterates_in_order() {
    let mut scope = RequestScope::new();
    assert!(scope.is_empty());
    scope.insert("org_id", "org_1");
    scope.insert("dept_id", "dept_9");
    scope.insert("org_id", "org_2");
    assert_eq!(scope.get("org_id"), Some("org_2"));
    assert_eq!(scope.get("missing"), None);
    let entries: Vec<_> = scope.iter().collect();
    assert_eq!(entries, vec![("org_id", "org_2"), ("dept_id", "dept_9")]);
}

struct PanickingProvider;

#[async_trait::async_trait]
impl ConnectionScopeProvider for PanickingProvider {
    async fn scope_settings(&self, _scope: &RequestScope) -> Result<Vec<ScopeSetting>, ScopeError> {
        panic!("must never be consulted in this binary");
    }
}

#[test]
fn an_empty_registry_discovers_nothing() {
    // No register_scope_provider! in this test binary: the memoized list is
    // empty, which is the no-op guarantee for existing installations.
    assert!(discover_scope_providers().is_empty());
    assert!(scope_providers().is_empty());
    // The trait object itself is constructible without registration.
    let _unregistered: SharedScopeProvider = Arc::new(PanickingProvider);
    assert!(scope_providers().is_empty());
}
