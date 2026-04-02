use systemprompt_extension::hlist::{Subset, TypeList};

struct Alpha;
struct Beta;
struct Gamma;

#[test]
fn empty_type_list_len_is_zero() {
    assert_eq!(<() as TypeList>::len(), 0);
}

#[test]
fn empty_type_list_is_empty() {
    assert!(<() as TypeList>::is_empty());
}

#[test]
fn empty_type_list_contains_nothing() {
    assert!(!<() as TypeList>::contains_type::<Alpha>());
}

#[test]
fn empty_type_list_type_ids_empty() {
    let ids = <() as TypeList>::type_ids();
    assert!(ids.is_empty());
}

#[test]
fn single_element_type_list_len() {
    assert_eq!(<(Alpha, ()) as TypeList>::len(), 1);
}

#[test]
fn single_element_type_list_not_empty() {
    assert!(!<(Alpha, ()) as TypeList>::is_empty());
}

#[test]
fn single_element_type_list_contains_self() {
    assert!(<(Alpha, ()) as TypeList>::contains_type::<Alpha>());
}

#[test]
fn single_element_type_list_does_not_contain_other() {
    assert!(!<(Alpha, ()) as TypeList>::contains_type::<Beta>());
}

#[test]
fn two_element_type_list_len() {
    assert_eq!(<(Alpha, (Beta, ())) as TypeList>::len(), 2);
}

#[test]
fn two_element_type_list_contains_both() {
    assert!(<(Alpha, (Beta, ())) as TypeList>::contains_type::<Alpha>());
    assert!(<(Alpha, (Beta, ())) as TypeList>::contains_type::<Beta>());
}

#[test]
fn two_element_type_list_does_not_contain_third() {
    assert!(!<(Alpha, (Beta, ())) as TypeList>::contains_type::<Gamma>());
}

#[test]
fn three_element_type_list_len() {
    assert_eq!(<(Alpha, (Beta, (Gamma, ()))) as TypeList>::len(), 3);
}

#[test]
fn three_element_type_list_type_ids_count() {
    let ids = <(Alpha, (Beta, (Gamma, ()))) as TypeList>::type_ids();
    assert_eq!(ids.len(), 3);
}

#[test]
fn type_ids_are_unique() {
    let ids = <(Alpha, (Beta, (Gamma, ()))) as TypeList>::type_ids();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 3);
}

#[test]
fn empty_is_subset_of_anything() {
    assert!(<() as Subset<()>>::is_subset_of());
    assert!(<() as Subset<(Alpha, ())>>::is_subset_of());
}

#[test]
fn single_element_subset_of_itself() {
    assert!(<(Alpha, ()) as Subset<(Alpha, ())>>::is_subset_of());
}

#[test]
fn single_element_subset_of_larger() {
    assert!(<(Alpha, ()) as Subset<(Alpha, (Beta, ()))>>::is_subset_of());
}

#[test]
fn two_element_subset_of_superset() {
    assert!(<(Alpha, (Beta, ())) as Subset<(Alpha, (Beta, (Gamma, ())))>>::is_subset_of());
}
