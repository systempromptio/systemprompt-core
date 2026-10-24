use std::sync::Arc;

use serde_json::{Value, json};
use systemprompt_ai::models::ArgsValidity;
use systemprompt_gateway::audit::tool_validity::{ToolValidityCache, catalog_key};
use systemprompt_wire::canonical::CanonicalTool;

fn tool(name: &str, input_schema: Value) -> CanonicalTool {
    CanonicalTool {
        anthropic_definition: None,
        name: name.into(),
        description: None,
        input_schema,
        cache_control: None,
    }
}

fn catalog(n: usize) -> Vec<CanonicalTool> {
    vec![tool(&format!("t{n}"), json!({"type": "object"}))]
}

#[test]
fn one_catalog_compiles_once() {
    let cache = ToolValidityCache::new(4);
    let tools = vec![tool(
        "read",
        json!({"type": "object", "required": ["path"]}),
    )];
    let first = cache.validators_for(&tools);
    let second = cache.validators_for(&tools);
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(cache.len(), 1);
    assert_eq!(
        first.check_raw("read", "{}").validity,
        ArgsValidity::Invalid
    );
}

#[test]
fn the_cache_is_bounded_and_evicts_the_oldest() {
    let cache = ToolValidityCache::new(2);
    let oldest = cache.validators_for(&catalog(0));
    cache.validators_for(&catalog(1));
    cache.validators_for(&catalog(2));
    assert_eq!(cache.len(), 2);
    assert!(!Arc::ptr_eq(&oldest, &cache.validators_for(&catalog(0))));
    assert_eq!(cache.len(), 2);
}

#[test]
fn an_empty_catalog_is_not_cached() {
    let cache = ToolValidityCache::new(2);
    let validators = cache.validators_for(&[]);
    assert!(validators.is_empty());
    assert!(cache.is_empty());
    assert_eq!(
        validators.check_raw("any", "{}").validity,
        ArgsValidity::UnknownTool
    );
}

#[test]
fn the_key_tracks_names_and_schemas() {
    let a = vec![tool("a", json!({"type": "object"}))];
    let b = vec![tool("b", json!({"type": "object"}))];
    let a2 = vec![tool("a", json!({"type": "string"}))];
    assert_eq!(catalog_key(&a), catalog_key(&a.clone()));
    assert_ne!(catalog_key(&a), catalog_key(&b));
    assert_ne!(catalog_key(&a), catalog_key(&a2));
    assert_eq!(catalog_key(&a).len(), 64);
}
