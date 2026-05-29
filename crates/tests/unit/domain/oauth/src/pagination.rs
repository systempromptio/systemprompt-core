//! Tests for the `Pagination` wire-format type.

use systemprompt_oauth::Pagination;

fn sample() -> Pagination {
    Pagination {
        page: 1,
        per_page: 20,
        total: 200,
        total_pages: 10,
    }
}

#[test]
fn pagination_fields_round_trip_through_serde() {
    let p = sample();
    let json = serde_json::to_string(&p).expect("serialize pagination");
    let back: Pagination = serde_json::from_str(&json).expect("deserialize pagination");

    assert_eq!(back.page, p.page);
    assert_eq!(back.per_page, p.per_page);
    assert_eq!(back.total, p.total);
    assert_eq!(back.total_pages, p.total_pages);
}

#[test]
fn pagination_serialize_contains_expected_keys() {
    let p = sample();
    let json = serde_json::to_string(&p).expect("serialize");

    assert!(json.contains("\"page\""));
    assert!(json.contains("\"per_page\""));
    assert!(json.contains("\"total\""));
    assert!(json.contains("\"total_pages\""));
}

#[test]
fn pagination_debug_is_derived() {
    let p = sample();
    let dbg = format!("{:?}", p);
    assert!(dbg.contains("Pagination"));
    assert!(dbg.contains("20"));
}

#[test]
fn pagination_copy_produces_independent_value() {
    let original = sample();
    let copied = original;
    assert_eq!(copied.page, original.page);
}

#[test]
fn pagination_clone_produces_independent_value() {
    let original = sample();
    let cloned = original;
    assert_eq!(cloned.total, original.total);
}

#[test]
fn pagination_first_page_zero_results() {
    let p = Pagination {
        page: 1,
        per_page: 50,
        total: 0,
        total_pages: 0,
    };
    let json = serde_json::to_string(&p).expect("serialize");
    let back: Pagination = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.total, 0);
    assert_eq!(back.total_pages, 0);
}

#[test]
fn pagination_large_total() {
    let p = Pagination {
        page: 999,
        per_page: 100,
        total: u32::MAX,
        total_pages: 42_949_673,
    };
    let json = serde_json::to_string(&p).expect("serialize");
    let back: Pagination = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.total, u32::MAX);
}
