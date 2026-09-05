use systemprompt_marketplace::{
    enabled_marketplaces, scope_to_marketplace, scope_to_union, union_include,
};
use systemprompt_models::services::MarketplaceMemberKind;

use crate::helpers::{config_with, include as include_ref, marketplace};

#[test]
fn enabled_marketplaces_is_empty_without_any() {
    let config = config_with(vec![]);
    assert!(enabled_marketplaces(&config).is_empty());
}

#[test]
fn enabled_marketplaces_lists_every_enabled_one_sorted_by_id() {
    let config = config_with(vec![marketplace("beta"), marketplace("alpha")]);
    let ids: Vec<&str> = enabled_marketplaces(&config)
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(ids, vec!["alpha", "beta"]);
}

#[test]
fn enabled_marketplaces_skips_disabled_ones() {
    let mut disabled = marketplace("beta");
    disabled.enabled = false;
    let config = config_with(vec![marketplace("alpha"), disabled]);
    let ids: Vec<&str> = enabled_marketplaces(&config)
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(ids, vec!["alpha"]);
}

#[test]
fn union_include_is_none_without_marketplaces() {
    assert!(union_include(&[], MarketplaceMemberKind::Plugins).is_none());
}

#[test]
fn union_include_merges_every_marketplaces_list() {
    let mut alpha = marketplace("alpha");
    alpha.plugins = include_ref(&["one"]);
    let mut beta = marketplace("beta");
    beta.plugins = include_ref(&["two"]);
    let union = union_include(&[&alpha, &beta], MarketplaceMemberKind::Plugins)
        .expect("both marketplaces name a list");
    assert!(union.contains("one") && union.contains("two"));
    assert_eq!(union.len(), 2);
}

#[test]
fn union_include_is_none_when_any_marketplace_means_all() {
    let mut alpha = marketplace("alpha");
    alpha.plugins = include_ref(&["one"]);
    let beta = marketplace("beta");
    assert!(
        union_include(&[&alpha, &beta], MarketplaceMemberKind::Plugins).is_none(),
        "an empty include means all, so the union is unbounded",
    );
}

#[test]
fn scope_to_union_passes_everything_through_when_unbounded() {
    let items = vec!["alpha".to_owned(), "beta".to_owned()];
    let scoped = scope_to_union(items.clone(), None, |s| s.as_str());
    assert_eq!(scoped, items);
}

#[test]
fn scope_to_union_keeps_only_named_ids() {
    let items = vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()];
    let include = ["alpha".to_owned(), "gamma".to_owned()].into_iter().collect();
    let scoped = scope_to_union(items, Some(&include), |s| s.as_str());
    assert_eq!(scoped, vec!["alpha".to_owned(), "gamma".to_owned()]);
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
