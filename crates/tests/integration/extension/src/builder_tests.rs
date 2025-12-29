//! Tests for the extension builder.

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

#[test]
fn test_builder_with_satisfied_deps() {
    let registry = ExtensionBuilder::new()
        .extension(AuthExtension)
        .extension(BlogExtension)
        .build()
        .expect("build should succeed");

    assert!(registry.has("auth"));
    assert!(registry.has("blog"));
    assert_eq!(registry.len(), 2);
}

#[test]
fn test_builder_empty() {
    let registry = ExtensionBuilder::new()
        .build()
        .expect("empty build should succeed");
    assert!(registry.is_empty());
}

#[test]
fn test_duplicate_extension_rejected() {
    let result = ExtensionBuilder::new()
        .extension(AuthExtension)
        .extension(AuthExtension)
        .build();

    assert!(result.is_err());
}
