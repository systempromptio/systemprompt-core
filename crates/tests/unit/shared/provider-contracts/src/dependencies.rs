//! Coverage for `Dependencies` and `MissingDependency`.

use std::sync::Arc;

use systemprompt_provider_contracts::{Dependencies, MissingDependency, ProviderError};

#[test]
fn an_empty_map_reports_the_requested_type() {
    let err = Dependencies::new().get::<String>().unwrap_err();
    assert!(err.type_name().contains("String"));
    assert!(err.to_string().contains("String"));
}

#[test]
fn inserting_a_type_again_replaces_the_value() {
    let mut deps = Dependencies::new().with(1u16);
    deps.insert(2u16);
    assert_eq!(deps.get::<u16>(), Ok(&2));
}

#[test]
fn clones_share_the_inserted_values() {
    let shared = Arc::new(5u8);
    let deps = Dependencies::new().with(Arc::clone(&shared));
    let cloned = deps.clone();
    let from_clone = cloned.get::<Arc<u8>>().expect("present in the clone");
    assert!(Arc::ptr_eq(from_clone, &shared));
}

#[test]
fn debug_lists_type_names_sorted() {
    let deps = Dependencies::new().with(3u32).with(1u8);
    assert_eq!(
        format!("{deps:?}"),
        "Dependencies { types: [\"u32\", \"u8\"], reserved: [], sealed: [] }"
    );
}

#[test]
fn missing_dependency_converts_into_provider_error() {
    let err: MissingDependency = Dependencies::new().get::<i8>().unwrap_err();
    let provider = ProviderError::from(err);
    assert!(matches!(provider, ProviderError::MissingDependency(e) if e.type_name() == "i8"));
}

#[test]
fn withholding_sealed_removes_every_sealed_entry_and_keeps_the_open_ones() {
    let deps = Dependencies::new().with(1u16).with_sealed(Arc::new(2u32));
    let withheld = deps.clone().withholding_sealed();
    assert_eq!(withheld.get::<u16>(), Ok(&1));
    assert!(withheld.get::<Arc<u32>>().is_err());
    assert!(
        deps.get::<Arc<u32>>().is_ok(),
        "the original keeps its sealed entry"
    );
}

#[test]
fn a_withheld_sealed_type_cannot_be_provided_again() {
    let mut withheld = Dependencies::new()
        .with_sealed(Arc::new(2u32))
        .withholding_sealed();
    let owner = systemprompt_identifiers::ExtensionId::new("probe");
    assert!(withheld.provide(&owner, Arc::new(Arc::new(3u32))).is_err());
}
