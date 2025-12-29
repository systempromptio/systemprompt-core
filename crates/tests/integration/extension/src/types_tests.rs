//! Tests for `ExtensionType` and Dependencies traits.

use systemprompt_extension::prelude::*;

#[derive(Default, Debug)]
struct AuthExtension;

impl ExtensionType for AuthExtension {
    const ID: &'static str = "auth";
    const NAME: &'static str = "Authentication";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for AuthExtension {}

#[derive(Default, Debug)]
struct BlogExtension;

impl ExtensionType for BlogExtension {
    const ID: &'static str = "blog";
    const NAME: &'static str = "Blog";
    const VERSION: &'static str = "1.0.0";
}

impl Dependencies for BlogExtension {
    type Deps = (AuthExtension, ());
}

#[derive(Default, Debug)]
struct AnalyticsExtension;

impl ExtensionType for AnalyticsExtension {
    const ID: &'static str = "analytics";
    const NAME: &'static str = "Analytics";
    const VERSION: &'static str = "1.0.0";
}

impl Dependencies for AnalyticsExtension {
    type Deps = (BlogExtension, (AuthExtension, ()));
}

#[derive(Default, Debug)]
struct CustomPriorityExtension;

impl ExtensionType for CustomPriorityExtension {
    const ID: &'static str = "custom-priority";
    const NAME: &'static str = "Custom Priority";
    const VERSION: &'static str = "2.0.0";
    const PRIORITY: u32 = 50;
}

impl NoDependencies for CustomPriorityExtension {}

#[test]
fn test_extension_type_metadata() {
    assert_eq!(AuthExtension::ID, "auth");
    assert_eq!(AuthExtension::NAME, "Authentication");
    assert_eq!(AuthExtension::VERSION, "1.0.0");
    assert_eq!(AuthExtension::PRIORITY, 100);
}

#[test]
fn test_extension_type_custom_priority() {
    assert_eq!(CustomPriorityExtension::PRIORITY, 50);
}

#[test]
fn test_extension_type_id() {
    use std::any::TypeId;
    let type_id = AuthExtension::type_id();
    assert_eq!(type_id, TypeId::of::<AuthExtension>());
}

#[test]
fn test_dependency_list_empty() {
    let ids = <() as DependencyList>::dependency_ids();
    assert!(ids.is_empty());
}

#[test]
fn test_dependency_list_single() {
    type Deps = (AuthExtension, ());
    let ids = <Deps as DependencyList>::dependency_ids();
    assert_eq!(ids, vec!["auth"]);
}

#[test]
fn test_dependency_list_multiple() {
    type Deps = (BlogExtension, (AuthExtension, ()));
    let ids = <Deps as DependencyList>::dependency_ids();
    assert_eq!(ids, vec!["blog", "auth"]);
}

#[test]
fn test_no_dependencies_impl() {
    type Deps = <AuthExtension as Dependencies>::Deps;
    let ids = <Deps as DependencyList>::dependency_ids();
    assert!(ids.is_empty());
}

#[test]
fn test_dependencies_with_deps() {
    type Deps = <BlogExtension as Dependencies>::Deps;
    let ids = <Deps as DependencyList>::dependency_ids();
    assert_eq!(ids, vec!["auth"]);
}

#[test]
fn test_chained_dependencies() {
    type Deps = <AnalyticsExtension as Dependencies>::Deps;
    let ids = <Deps as DependencyList>::dependency_ids();
    assert_eq!(ids, vec!["blog", "auth"]);
}

#[test]
fn test_missing_dependency_debug() {
    let missing = MissingDependency {
        extension_id: "auth",
        extension_name: "Authentication",
    };
    let debug_str = format!("{:?}", missing);
    assert!(debug_str.contains("auth"));
    assert!(debug_str.contains("Authentication"));
}
