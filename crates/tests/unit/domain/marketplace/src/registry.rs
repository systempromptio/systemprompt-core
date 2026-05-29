use systemprompt_marketplace::discover_filters;

#[test]
fn discover_filters_returns_stable_sorted_slice() {
    let filters = discover_filters();
    for window in filters.windows(2) {
        assert!(
            window[0].priority >= window[1].priority,
            "filters must be sorted by descending priority",
        );
    }
}

#[test]
fn discover_filters_no_panic_on_repeated_calls() {
    let first = discover_filters().len();
    let second = discover_filters().len();
    assert_eq!(first, second, "discover_filters must be idempotent");
}
