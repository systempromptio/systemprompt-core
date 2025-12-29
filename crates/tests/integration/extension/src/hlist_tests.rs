//! Tests for type-level `HList` operations.

use systemprompt_extension::hlist::{Contains, Subset, TypeList};

struct A;
struct B;
struct C;

#[test]
fn test_type_list_contains_empty() {
    assert!(!<() as TypeList>::contains_type::<A>());
    assert!(!<() as TypeList>::contains_type::<B>());
}

#[test]
fn test_type_list_contains_single() {
    type List = (A, ());
    assert!(<List as TypeList>::contains_type::<A>());
    assert!(!<List as TypeList>::contains_type::<B>());
}

#[test]
fn test_type_list_contains_multiple() {
    type List = (A, (B, ()));
    assert!(<List as TypeList>::contains_type::<A>());
    assert!(<List as TypeList>::contains_type::<B>());
    assert!(!<List as TypeList>::contains_type::<C>());
}

#[test]
fn test_type_list_type_ids_empty() {
    let ids = <() as TypeList>::type_ids();
    assert!(ids.is_empty());
}

#[test]
fn test_type_list_type_ids_single() {
    use std::any::TypeId;
    type List = (A, ());
    let ids = <List as TypeList>::type_ids();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], TypeId::of::<A>());
}

#[test]
fn test_type_list_type_ids_multiple() {
    use std::any::TypeId;
    type List = (A, (B, ()));
    let ids = <List as TypeList>::type_ids();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], TypeId::of::<A>());
    assert_eq!(ids[1], TypeId::of::<B>());
}

fn assert_subset<S: Subset<T>, T: TypeList>() {}

#[test]
fn test_empty_subset_of_anything() {
    assert_subset::<(), ()>();
    assert_subset::<(), (A, ())>();
    assert_subset::<(), (A, (B, ()))>();
}

#[test]
fn test_subset_of_self() {
    assert_subset::<(A, ()), (A, ())>();
    assert_subset::<(A, (B, ())), (A, (B, ()))>();
}

#[test]
fn test_subset_of_superset() {
    assert_subset::<(A, ()), (A, (B, ()))>();
    assert_subset::<(B, ()), (A, (B, ()))>();
}

fn assert_contains<L: Contains<T>, T: 'static>() {}

#[test]
fn test_contains_trait_head_only() {
    // Contains trait only works for head position due to Rust coherence rules
    assert_contains::<(A, ()), A>();
    assert_contains::<(A, (B, ())), A>();
    assert_contains::<(B, (A, ())), B>();
    // For non-head positions, use TypeList::contains_type() instead
}

#[test]
fn test_subset_runtime_verification() {
    // Subset::is_subset_of() provides runtime verification
    assert!(<() as Subset<()>>::is_subset_of());
    assert!(<() as Subset<(A, ())>>::is_subset_of());
    assert!(<(A, ()) as Subset<(A, ())>>::is_subset_of());
    assert!(<(A, ()) as Subset<(A, (B, ()))>>::is_subset_of());
    assert!(<(B, ()) as Subset<(A, (B, ()))>>::is_subset_of());
    assert!(<(A, (B, ())) as Subset<(A, (B, ()))>>::is_subset_of());

    // These would fail runtime verification (but still compile due to blanket impl)
    assert!(!<(C, ()) as Subset<(A, (B, ()))>>::is_subset_of());
}

// =============================================================================
// TypeList len() and is_empty() Tests
// =============================================================================

#[test]
fn test_type_list_len_empty() {
    assert_eq!(<() as TypeList>::len(), 0);
}

#[test]
fn test_type_list_len_single() {
    type List = (A, ());
    assert_eq!(<List as TypeList>::len(), 1);
}

#[test]
fn test_type_list_len_multiple() {
    type List = (A, (B, (C, ())));
    assert_eq!(<List as TypeList>::len(), 3);
}

#[test]
fn test_type_list_is_empty() {
    assert!(<() as TypeList>::is_empty());
    assert!(!<(A, ()) as TypeList>::is_empty());
    assert!(!<(A, (B, ())) as TypeList>::is_empty());
}
