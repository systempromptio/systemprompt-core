use systemprompt_marketplace::{active_marketplace, scope_to_marketplace};

use crate::helpers::{config_with, marketplace};

#[test]
fn active_marketplace_none_when_empty() {
    let config = config_with(vec![]);
    assert!(active_marketplace(&config).is_none());
}

#[test]
fn active_marketplace_some_when_single() {
    let config = config_with(vec![marketplace("solo")]);
    let active = active_marketplace(&config).expect("single marketplace is active");
    assert_eq!(active.id.as_str(), "solo");
}

#[test]
fn active_marketplace_picks_one_when_many() {
    let config = config_with(vec![marketplace("alpha"), marketplace("beta")]);
    let active = active_marketplace(&config).expect("fail-open picks a marketplace");
    assert!(matches!(active.id.as_str(), "alpha" | "beta"));
}

#[test]
fn scope_filters_to_included_ids() {
    let items = vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()];
    let include = vec!["alpha".to_owned(), "gamma".to_owned()];
    let scoped = scope_to_marketplace(items, &include, |s| s.as_str());
    assert_eq!(scoped, vec!["alpha".to_owned(), "gamma".to_owned()]);
}

#[test]
fn scope_empty_include_returns_all() {
    let items = vec!["alpha".to_owned(), "beta".to_owned()];
    let include: Vec<String> = vec![];
    let scoped = scope_to_marketplace(items.clone(), &include, |s| s.as_str());
    assert_eq!(scoped, items);
}

#[test]
fn scope_drops_nonexistent_include_id() {
    let items = vec!["alpha".to_owned(), "beta".to_owned()];
    let include = vec!["alpha".to_owned(), "does-not-exist".to_owned()];
    let scoped = scope_to_marketplace(items, &include, |s| s.as_str());
    assert_eq!(scoped, vec!["alpha".to_owned()]);
}

#[test]
fn scope_preserves_input_order() {
    let items = vec!["c".to_owned(), "a".to_owned(), "b".to_owned()];
    let include = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
    let scoped = scope_to_marketplace(items, &include, |s| s.as_str());
    assert_eq!(scoped, vec!["c".to_owned(), "a".to_owned(), "b".to_owned()]);
}
